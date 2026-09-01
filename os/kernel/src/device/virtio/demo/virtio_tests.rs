use log::{info, error, warn};
use super::super::{virtio_gpu, virtio_rng};
use virtio::device::sound::{PcmFormat, PcmRate, PcmFeatures};

pub fn play_pcm_file() {

    // 1) PCM-Daten einbetten (Datei liegt neben virtio_tests.rs)
    const PCM_BYTES: &[u8] = include_bytes!("test_song.pcm");

    // Ziel-Parameter
    let mut target_rate  = PcmRate::Rate44100;
    let mut target_fmt   = PcmFormat::S16;
    let mut target_ch    = 2u8;

    // Device-Infos abfragen
    let (sound_mutex, stream_id, rates_ok, fmts_ok, ch_range_ok) = match super::super::virtio_sound() {
        Some(m) => {
            // nur zum Abfragen kurz locken
            let mut dev = m.lock();
            let outs = match dev.output_streams() {
                Ok(v) if !v.is_empty() => v,
                Ok(_) => { warn!("No output streams."); return; }
                Err(e) => { error!("output_streams() failed: {:?}", e); return; }
            };
            let sid = outs[0];

            // Fähigkeiten lesen
            let rates_ok = dev.rates_supported(sid);
            let fmts_ok  = dev.formats_supported(sid);
            let ch_ok    = dev.channel_range_supported(sid);
            drop(dev); // Lock sofort weg

            (m, sid, rates_ok, fmts_ok, ch_ok)
        }
        None => { warn!("VirtIO Sound device not found."); return; }
    };

    // Parameter anpassen (Lock nicht nötig)
    if let Ok(rates) = rates_ok {
        if !rates.contains(target_rate.into()) {
            // simpler Fallback
            target_rate = if rates.contains(PcmRate::Rate48000.into()) { PcmRate::Rate48000 }
                          else if rates.contains(PcmRate::Rate44100.into()) { PcmRate::Rate44100 }
                          else { PcmRate::Rate32000 };
        }
    }
    if let Ok(fmts) = fmts_ok {
        if !fmts.contains(target_fmt.into()) {
            target_fmt = if fmts.contains(PcmFormat::S16.into()) { PcmFormat::S16 }
                         else if fmts.contains(PcmFormat::S24.into()) { PcmFormat::S24 }
                         else { PcmFormat::S32 };
        }
    }
    if let Ok(range) = ch_range_ok {
        if !range.contains(&target_ch) {
            target_ch = *range.start(); // Kanalzahl
        }
    }

    // Bytes pro Sample
    let bytes_per_sample = match target_fmt {
        PcmFormat::S16 => 2,
        PcmFormat::S24 => 3,
        PcmFormat::S32 => 4,
        _ => 2, // Default
    };
    let frame_size = bytes_per_sample * (target_ch as usize);

    // Periodengröße Frame-aligned (z. B. 4096 B)
    let mut period_bytes = 4096usize;
    if period_bytes % frame_size != 0 {
        period_bytes = ((period_bytes + frame_size - 1) / frame_size) * frame_size;
    }

    // Größerer Ringpuffer (z. B. 8 Perioden)
    let buffer_bytes = (period_bytes * 8) as u32;
    let period_bytes_u32 = period_bytes as u32;

    info!("PCM config -> rate={:?}, fmt={:?}, ch={}, period={} B, buffer={} B",
        target_rate, target_fmt, target_ch, period_bytes, buffer_bytes);

    // Stream konfigurieren (je Schritt kurz locken)
    {
        let mut dev = sound_mutex.lock();
        if let Err(e) = dev.pcm_set_params(stream_id, buffer_bytes, period_bytes_u32,
                                           PcmFeatures::empty(), target_ch, target_fmt, target_rate) {
            error!("pcm_set_params failed: {:?}", e);
            return;
        }
    }
    { let mut dev = sound_mutex.lock(); if let Err(e) = dev.pcm_prepare(stream_id) { error!("prepare failed: {:?}", e); return; } }
    { let mut dev = sound_mutex.lock(); if let Err(e) = dev.pcm_start(stream_id)   { error!("start failed: {:?}", e);   return; } }

    // Daten streamen, PCM-Daten in periodengroße Stücke schneiden (überstehende ignorieren)
    let total_len = PCM_BYTES.len() - (PCM_BYTES.len() % period_bytes);
    let mut submitted_tokens: alloc::collections::VecDeque<u16> = alloc::collections::VecDeque::new();

    // "In-Flight"-Fenster damit die Queue gefüllt bleibt
    const IN_FLIGHT: usize = 8; // zu 16?

    let mut offset = 0usize;
    while offset < total_len || !submitted_tokens.is_empty() {
        // 1) so lange Tokens einsenden bis Fenster voll oder Daten alle
        while submitted_tokens.len() < IN_FLIGHT && offset < total_len {
            let chunk = &PCM_BYTES[offset .. offset + period_bytes];
            let token = { let mut dev = sound_mutex.lock(); dev.pcm_xfer_nb(stream_id, chunk).expect("xfer_nb") };
            submitted_tokens.push_back(token);
            offset += period_bytes;
        }

        // 2) Notifications leeren
        { 
            let mut dev = sound_mutex.lock();
            while let Ok(Some(_n)) = dev.latest_notification() {
            }
        }

        // 3) So viele Completions wie möglich quittieren
        {
            let mut dev = sound_mutex.lock();
            while let Some(tok) = submitted_tokens.pop_front() {
                if dev.pcm_xfer_ok(tok).is_err() {
                    submitted_tokens.push_front(tok);
                    break;
                }
            }
        }

        core::hint::spin_loop(); // mini-yield
    }

    // Stop & Release
    { let mut dev = sound_mutex.lock(); if let Err(e) = dev.pcm_stop(stream_id)    { warn!("stop failed: {:?}", e); } }
    { let mut dev = sound_mutex.lock(); if let Err(e) = dev.pcm_release(stream_id) { warn!("release failed: {:?}", e); } }

    info!("PCM file playback finished");
}

pub fn test_rng() {
    if let Some(rng_mutex) = virtio_rng() {
        let mut buffer = [0u8; 10];

        let result = {
            let mut rng = rng_mutex.lock();
            rng.request_entropy(&mut buffer)
        }; //Lock freigeben

        match result {
            Ok(bytes) => {
                info!("Successfully received {} random bytes: {:?}", bytes, &buffer[..bytes]);
            }
            Err(e) => {
                error!("Failed to get random bytes from VirtIO RNG: {:?}", e);
            }
        }
    } else {
        info!("VirtIO RNG device not found or not initialized.");
    }
}

pub fn test_virgl() {
    if let Some(gpu_mutex) = virtio_gpu() {
        let mut gpu = gpu_mutex.lock();
        gpu.test_virgl(); // Funktion in gpu.rs aufrufen
    } else {
        info!("VirtIO GPU device not found or not initialized. Skipping test.");
    }
}