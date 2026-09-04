// Ich habe beim Erstellen des Codes KI als Hilfsmittel verwendet.
// Dies beeinhaltet die syntaktische Ausformulierung in Rust, Code Refactoring, sowie das Debuggen und die Fehlersuche


#![no_std]
#[allow(unused_imports)]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use concurrent::thread;
use naming::shared_types::{FileType, OpenOptions, SeekOrigin};
use spin::Mutex;
use syscall::return_vals::Errno;
use runtime::*;
use serde::Deserialize;
use terminal::println;
use log::error;
use time::systime;

const HEARTBEAT_DIR: &str = "/heartbeat";           // Ort für die Heartbeat-Dateien
const PROC_SERVICES_PATH: &str = "/proc/services";  // Ort für die Service-Status-Datei, die vom TaskManager gelesen wird
const HEARTBEAT_INTERVAL_MS: usize = 2_000;         // Wie oft Heartbeats gesendet werden
const WATCHDOG_TICK_INTERVAL_MS: usize = 1_000;    // Wie oft Heartbeats geprüft werden
const HEARTBEAT_TIMEOUT_MS: usize = 5_000;         // Wie alt ein Heartbeat sein darf, bevor der Watchdog Alarm schlägt

#[derive(Debug, Clone, Copy)]
struct HeartbeatState {     // Merken uns für jeden Service das letzte Signal + ob schon ein Alarm/Neustart durchgeführt wurde + Datei handle
    last_seen_ms: usize,
    last_marker: u64,
    killed: bool,
    thread_id: usize,
    handle: Option<usize>,
}

type HeartbeatMap = BTreeMap<String, HeartbeatState>;   // Service Name -> HeartbeatState

// Stellt den aktuellen Status aller Services dar. Wird im TaskManager verwendet um Statusinformationen anzuzeigen
#[derive(Debug, Clone)]
struct ServiceStatusEntry {
    binary: String,
    state: ServiceState,
    heartbeat: bool,
    restart: RestartPolicy,
}

type ServiceStatusMap = BTreeMap<String, ServiceStatusEntry>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootTarget {   // In boot.rs wird zwischen Terminal und GUI unterschieden beim booten
    Terminal,
    Gui,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceState {     // Alle möglichen Zustände eines Service
    Disabled,
    Offline,
    Online,
    Degraded,
    Maintenance,
}

impl ServiceState {
    // String Mappings für die Ausgabe beim TaskManager
    const fn as_str(self) -> &'static str {
        match self {
            ServiceState::Disabled => "disabled",
            ServiceState::Offline => "offline",
            ServiceState::Online => "online",
            ServiceState::Degraded => "degraded",
            ServiceState::Maintenance => "maintenance",
        }
    }
}

impl RestartPolicy {
    const fn as_str(self) -> &'static str {
        match self {
            RestartPolicy::Never => "never",
            RestartPolicy::Always => "always",
            RestartPolicy::OnFailure => "on_failure",
        }
    }
}

fn is_running_state(state: ServiceState) -> bool {  // Im Solaris Paper ist dies als Online oder Degraded definiert. Für meine Implementierung macht aber nur Online Sinn.
    if state == ServiceState::Online  {
        true
    } 
    else {
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")] // Automatisch in snake_case umwandeln fürs leichteres parsen
enum RestartPolicy {    // Wie umgehen mit Service-Ausfällen?
    Never,
    Always,
    OnFailure,
}



#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ManifestTarget {
    Terminal,
    Gui,
    Any,
}

// Default Werte für Manfifeste, falls sie in der JSON Datei nicht explizit gesetzt wurden
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}


#[derive(Debug, Clone, Deserialize)]
struct ServiceManifestJson {
    name: String,
    binary: String,

    #[serde(default)]
    args: Vec<String>,

    #[serde(default = "default_true")]
    enabled: bool,

    // Leer bedeutet "any".
    #[serde(default)]
    targets: Vec<ManifestTarget>,

    #[serde(default = "default_restart_policy")]
    restart: RestartPolicy,

    #[serde(default = "default_false")]
    heartbeat: bool,

    #[serde(default)]
    require_all: Vec<String>,
    #[serde(default)]
    require_any: Vec<String>,
    #[serde(default)]
    optional_all: Vec<String>,
    #[serde(default)]
    exclude_all: Vec<String>,
}

impl ServiceManifestJson {
    fn into_manifest(self) -> ServiceManifest {
        ServiceManifest {
            name: self.name,
            binary: self.binary,
            args: self.args,
            enabled: self.enabled,
            targets: self.targets,
            restart: self.restart,
            heartbeat: self.heartbeat,
            require_all: self.require_all,
            require_any: self.require_any,
            optional_all: self.optional_all,
            exclude_all: self.exclude_all,
        }
    }
}

#[derive(Debug, Clone)]
struct ServiceManifest {
    name: String,
    binary: String,
    args: Vec<String>,
    enabled: bool,
    targets: Vec<ManifestTarget>,
    restart: RestartPolicy,
    heartbeat: bool,
    require_all: Vec<String>,
    require_any: Vec<String>,
    optional_all: Vec<String>,
    exclude_all: Vec<String>,
}

#[derive(Debug, Clone)]
struct Service {
    manifest: ServiceManifest,
    state: ServiceState,
}

fn render_service_snapshot(statuses: &ServiceStatusMap) -> String {
    let mut content = String::new();
    content.push_str(&alloc::format!("{:<18} {:<12} {:<18} {:<10} {:<12}\n",
        "SERVICE", "STATE", "BINARY", "HEARTBEAT", "RESTART"));

    for (name, status) in statuses.iter() {
        content.push_str(&alloc::format!("{:<18} {:<12} {:<18} {:<10} {:<12}\n",
            name, status.state.as_str(),status.binary, if status.heartbeat {"yes"} else {"no"}, status.restart.as_str(),));
    }

    content
}

// Schreibt die aktuelle ServiceStatusMap in /proc/services, damit andere Prozesse sie lesen können (z.B. TaskManager)
fn publish_service_snapshot(service_states: &Arc<Mutex<ServiceStatusMap>>) {
    let snapshot = {
        let statuses = service_states.lock();
        render_service_snapshot(&statuses)
    };

    if let Ok(fh) = naming::open(PROC_SERVICES_PATH, OpenOptions::WRITEONLY) {
        let _ = naming::seek(fh, 0, SeekOrigin::Start);
        let _ = naming::write(fh, snapshot.as_bytes());
        let _ = naming::close(fh);
    }
}

fn update_service_state(service_states: &Arc<Mutex<ServiceStatusMap>>, name: &str, state: ServiceState) {
    {
        let mut statuses = service_states.lock();
        if let Some(entry) = statuses.get_mut(name) {
            entry.state = state;
        }
    }

    publish_service_snapshot(service_states);
}

// Initialisiert die ServiceStatusMap für den TaskManager
fn init_service_status_map(services: &[Service]) -> Arc<Mutex<ServiceStatusMap>> {
    let mut statuses = BTreeMap::new();
    for service in services {
        statuses.insert(service.manifest.name.clone(),ServiceStatusEntry {binary: service.manifest.binary.clone(), state: service.state, heartbeat: service.manifest.heartbeat, restart: service.manifest.restart});
    }

    let service_states = Arc::new(Mutex::new(statuses));
    publish_service_snapshot(&service_states);
    service_states
}

fn should_restart(policy: RestartPolicy, exit_code: usize) -> bool {
    match policy {
        RestartPolicy::Never => false,
        RestartPolicy::Always => true,
        RestartPolicy::OnFailure => exit_code != 0,
    }
}

// Hängt die Heartbeat argumente an, falls heartbeat auf true ist
fn build_effective_args(manifest: &ServiceManifest) -> Vec<String> {
    let mut args = manifest.args.clone();
    if manifest.heartbeat {
        let hb_path = alloc::format!("{}/{}", HEARTBEAT_DIR, manifest.name);
        args.push(alloc::format!("--service-name={}", manifest.name));
        args.push(alloc::format!("--heartbeat-path={}", hb_path));
        args.push(alloc::format!("--heartbeat-ms={}", HEARTBEAT_INTERVAL_MS));
    }
    args
}
// Heartsbeats registrieren oder entfernen
fn heartbeat_register(heartbeats: &Arc<Mutex<HeartbeatMap>>, name: &str, thread_id: usize, handle: Option<usize>) {
    let mut map = heartbeats.lock();
    map.insert(name.to_string(), HeartbeatState {last_seen_ms: systime().num_milliseconds() as usize, last_marker: u64::MAX, killed: false, thread_id, handle});
}

fn heartbeat_remove(heartbeats: &Arc<Mutex<HeartbeatMap>>, name: &str) {
    let mut map = heartbeats.lock();
    if let Some(state) = map.remove(name) {
        if let Some(handle) = state.handle {
            let _ = naming::close(handle);
        }
    }
}

fn ensure_heartbeat_file(service_name: &str) -> String {
    let hb_path = alloc::format!("{}/{}", HEARTBEAT_DIR, service_name);
    let _ = naming::touch(&hb_path);        // TempFS nutzen (Dateien)
    hb_path
}

// Prüft die Heartbeats aller Services (welche Heartbeats aktiviert haben). Wenn ein Service zu lange kein Heartbeat gesendet hat, wird er entweder gekillt (bei RestartPolicy Always oder OnFailure) oder in den Maintenance Zustand versetzt (bei RestartPolicy Never)
fn start_watchdog(heartbeats: Arc<Mutex<HeartbeatMap>>, service_states: Arc<Mutex<ServiceStatusMap>>) {
    let _ = thread::create(move || {
        let mut buf = [0u8; 8];

        loop {
            thread::sleep(WATCHDOG_TICK_INTERVAL_MS);
            let now = systime().num_milliseconds() as usize;

            let mut missed: Vec<(String, usize, usize)> = Vec::new();
            let mut map = heartbeats.lock();
            for (name, state) in map.iter_mut() {
                if let Some(handle) = state.handle {
                    let _ = naming::seek(handle, 0, SeekOrigin::Start);
                    if let Ok(n) = naming::read(handle, &mut buf) {
                        if n == buf.len() {
                            let marker = u64::from_le_bytes(buf);
                            if marker != state.last_marker {
                                //println!("Watchdog hat neuen Marker von {} gelesen: {}", name, marker);
                                state.last_marker = marker;
                                state.last_seen_ms = now;
                                state.killed = false;
                            }
                        }
                    }
                }

                let age = now.saturating_sub(state.last_seen_ms);
                if age >= HEARTBEAT_TIMEOUT_MS && !state.killed {
                    missed.push((name.clone(), age / 1000, state.thread_id));
                    state.killed = true;
                }
            }
            drop(map);

            for (name, age, thread_id) in missed {
                error!("svc: watchdog missed heartbeat from {} (age={}s)", name, age);
                //println!("svc: watchdog missed heartbeat from {} (age={}s)", name, age);
                update_service_state(&service_states, &name, ServiceState::Degraded);
                //println!("svc: killing {} (tid={}) after missed heartbeat", name, thread_id);
                thread::Thread::from_id(thread_id).kill();
            }
        }
    });
}

// Service starten und überwachen. Wenn er abstürzt, wird er je nach RestartPolicy neu gestartet oder in den Maintenance Zustand versetzt
fn supervise_service(manifest: ServiceManifest, mut child: thread::Thread, heartbeats: Arc<Mutex<HeartbeatMap>>, service_states: Arc<Mutex<ServiceStatusMap>>) {
    loop {
        let exit_code = match child.join() {
            Ok(code) => code,
            Err(e) => {
                println!("svc: {} join failed (err={:?})", manifest.name, e);
                1
            }
        };

        //println!("svc: {} exited (code={})", manifest.name, exit_code);

        if manifest.heartbeat {
            heartbeat_remove(&heartbeats, &manifest.name);
        }

        let restart = should_restart(manifest.restart, exit_code);
        if restart || exit_code == 0 {
            update_service_state(&service_states, &manifest.name, ServiceState::Offline);
        } 
        else {
            update_service_state(&service_states, &manifest.name, ServiceState::Maintenance);
        }

        if !restart {
            //println!("svc: {} not restarting (policy={:?})", manifest.name, manifest.restart);
            break;
        }

        thread::sleep(200); // Kurz Verzögern, um Crash-Loops zu vermeiden

        //println!("svc: restarting {} -> {}", manifest.name, manifest.binary);
        let args = build_effective_args(&manifest);
        let args_vec: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        match thread::start_application(&manifest.binary, args_vec) {
            Some(new_child) => {
                //println!("svc: {} online (tid={})", manifest.name, new_child.id());
                child = new_child;
                update_service_state(&service_states, &manifest.name, ServiceState::Online);
                if manifest.heartbeat {
                    let heartbeat_path = ensure_heartbeat_file(&manifest.name);
                    let heartbeat_fh = naming::open(&heartbeat_path, OpenOptions::READONLY).ok();
                    heartbeat_register(&heartbeats, &manifest.name, child.id(), heartbeat_fh);
                }
            }
            None => {
                update_service_state(&service_states, &manifest.name, ServiceState::Maintenance);
                println!("svc: {} entered maintenance (restart exec failed)", manifest.name);
                break;
            }
        }
    }
}

fn service_state_by_name(services: &[Service], name: &str) -> Option<ServiceState> {
    services.iter().find(|s| s.manifest.name == name).map(|s| s.state)
}

// - exclude_all: nicht starten, solange irgendeiner davon läuft
// - require_all: erst starten, wenn alle davon laufen. Alle Dienste müssen im Online oder Degraded Zustand sein, damit der Service gestartet werden kann.
// - require_any: erst starten, wenn mindestens einer davon läuft (falls die Liste nicht leer ist)
// - optional_all: wie require_all, aber nur falls der Service überhaupt existiert; fehlt er, wird er ignoriert
// Prüft, ob die Abhängigkeiten für einen Service erfüllt sind
fn can_start_now(service: &Service, services: &[Service]) -> bool {
    for dep in &service.manifest.exclude_all {
        if let Some(dep_state) = service_state_by_name(services, dep.as_str()) {
            if is_running_state(dep_state) {
                return false;
            }
        }
    }

    for dep in &service.manifest.require_all {
        let Some(dep_state) = service_state_by_name(services, dep.as_str()) 
        else {
            return false;
        };
        if !is_running_state(dep_state) {
            return false;
        }
    }

    if !service.manifest.require_any.is_empty() {
        let mut any_running = false;
        for dep in &service.manifest.require_any {
            if let Some(dep_state) = service_state_by_name(services, dep.as_str()) {
                if is_running_state(dep_state) {
                    any_running = true;
                    break;
                }
            }
        }
        if !any_running {
            return false;
        }
    }

    for dep in &service.manifest.optional_all {
        let Some(dep_state) = service_state_by_name(services, dep.as_str()) else {
            continue;
        };
        if dep_state == ServiceState::Online || dep_state == ServiceState::Disabled || dep_state == ServiceState::Maintenance {
            continue;
        }
        return false;
    }

    true
}

fn start_service_once(service: &mut Service, heartbeats: &Arc<Mutex<HeartbeatMap>>, service_states: &Arc<Mutex<ServiceStatusMap>>) -> bool {
    //println!("svc: start {} -> {}", service.manifest.name, service.manifest.binary);
    let heartbeat_path = if service.manifest.heartbeat {
        Some(ensure_heartbeat_file(&service.manifest.name))
    } 
    else {
        None
    };
    let heartbeat_fh = heartbeat_path.as_ref().and_then(|path| naming::open(path, OpenOptions::READONLY).ok());
    let args = build_effective_args(&service.manifest);
    let args_vec: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    match thread::start_application(&service.manifest.binary, args_vec) {
        Some(child) => {
            let manifest = service.manifest.clone();
            let heartbeats = heartbeats.clone();
            let service_states = service_states.clone();

            service.state = ServiceState::Online;
            update_service_state(&service_states, &service.manifest.name, ServiceState::Online);
            //println!("svc: {} online (tid={})", service.manifest.name, child.id());

            if manifest.heartbeat {
                heartbeat_register(&heartbeats, &manifest.name, child.id(), heartbeat_fh);
            }

            let _ = thread::create(move || supervise_service(manifest, child, heartbeats, service_states));
            true
        }
        None => {
            if let Some(handle) = heartbeat_fh {
                let _ = naming::close(handle);
            }
            service.state = ServiceState::Maintenance;
            update_service_state(service_states, &service.manifest.name, ServiceState::Maintenance);
            println!("svc: {} entered maintenance (exec failed)", service.manifest.name);
            false
        }
    }
}

// Geht über alle Services und starte alle die, die dürfen (Offline + Abhängigkeiten erfüllt)
fn start_eligible_services(services: &mut [Service], start_order: &mut Vec<String>, heartbeats: &Arc<Mutex<HeartbeatMap>>, service_states: &Arc<Mutex<ServiceStatusMap>>) -> usize {
    let mut started = 0;
    for i in 0..services.len() {
        if services[i].state != ServiceState::Offline {
            continue;
        }

        if !can_start_now(&services[i], services) {
            continue;
        }

        let ok = start_service_once(&mut services[i], heartbeats, service_states);
        if ok {
            start_order.push(services[i].manifest.name.clone());
        }
        started += 1;
    }
    started
}

// Schleife, bis alle Services gestartet wurden. 
fn start_services_with_dependencies(services: &mut [Service], heartbeats: &Arc<Mutex<HeartbeatMap>>, service_states: &Arc<Mutex<ServiceStatusMap>>) {
    let mut start_order: Vec<String> = Vec::new();

    loop {
        let started_now = start_eligible_services(services, &mut start_order, heartbeats, service_states);
        if started_now == 0 {   // Nichts mehr gestaret, also entweder alle online oder Zyklus
            break;
        }
    }

    /* 
    if !start_order.is_empty() {
        println!("svc: start order ({}):", start_order.len());
        for name in &start_order {
            println!("svc:   {}", name);
        }
    }
    */

    let remaining_offline = services.iter().filter(|s| s.state == ServiceState::Offline).count();
    if remaining_offline != 0 {
        // Fehlermeldung sowohl im Terminal als auch im Kernel-Log ausgeben (Wenn Terminal nicht startet, trotzdem im Kernel-Log sichtbar)
        println!("svc: {} service(s) still offline after dependency start (missing deps or cycle)", remaining_offline);
        error!("svc: {} service(s) still offline after dependency start (missing deps or cycle)", remaining_offline);
    }
}

// Öffnet konkrete Manifest Datei
fn read_file_to_string(path: &str) -> Result<String, Errno> {   
    let fh = naming::open(path, OpenOptions::READONLY)?;
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 512];

    loop {
        let n = naming::read(fh, &mut buf)?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }

    let _ = naming::close(fh);

    match core::str::from_utf8(&out) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Ok(String::new()),
    }
}


// Manifest Dateien öffnen und Inhalt an parse_service_manifest übergeben
fn load_service_manifests() -> Vec<ServiceManifest> {
    let (dir_path, dir_fh) = match naming::open("/services", OpenOptions::DIRECTORY) {  // Manifest Dateien liegen in loader/initrd/services
        Ok(fh) => ("/services", fh),
        Err(e) => {
            println!("svc: no services directory (err={:?}); using built-in defaults", e);
            return Vec::new();
        }
    };

    let mut manifests: Vec<ServiceManifest> = Vec::new();
    loop {
        let entry = match naming::readdir(dir_fh) {
            Ok(v) => v,
            Err(e) => {
                println!("svc: readdir /services failed (err={:?})", e);
                break;
            }
        };

        let Some(entry) = entry else {
            break;
        };

        if entry.file_type != FileType::Regular || !entry.name.ends_with(".json") {
            continue;
        }

        let path = alloc::format!("{}/{}", dir_path, entry.name);
        match read_file_to_string(&path) {
            Ok(contents) => {
                match serde_json::from_str::<ServiceManifestJson>(&contents) {
                    Ok(m) => manifests.push(m.into_manifest()),
                    Err(e) => {
                        println!("svc: failed to parse {} (json err={})", path, e);
                    }
                }
            }
            Err(e) => {
                println!("svc: failed to read {} (err={:?})", path, e);
            }
        }
    }

    let _ = naming::close(dir_fh);
    manifests
}

// Hilfsmethode zum Debuggen, um die geladenen Manifeste zu überprüfen (wird später entfernt)
/* 
fn print_manifest(m: &ServiceManifest) {
    println!("svc: manifest name={} binary={} enabled={} restart={:?}", m.name, m.binary, m.enabled, m.restart);
    println!("svc:   targets={:?} args={} require_all={} require_any={} optional_all={} exclude_all={}", m.targets, m.args.len(), m.require_all.len(), m.require_any.len(), m.optional_all.len(), m.exclude_all.len());
}
*/

fn parse_boot_target() -> BootTarget {
    let mut args = env::args();
    let _ = args.next();

    for arg in args {
        match arg.as_str() {
            "--gui" => return BootTarget::Gui,
            "--terminal" => return BootTarget::Terminal,
            _ => {
                println!("service_manager: ignoring unknown arg: {}", arg);
            }
        }
    }

    BootTarget::Terminal
}

#[unsafe(no_mangle)]
pub fn main() {
    terminal::init_logger();

    //println!("D3OS service_manager starting...");

    let boot_target = parse_boot_target();
    //println!("service_manager: boot_target={:?}", boot_target);

    let manifests = load_service_manifests();
    //println!("svc: loaded {} manifest(s)", manifests.len());
    //for m in &manifests {
        //print_manifest(m);
    //}

    let mut services: Vec<Service> = if !manifests.is_empty() {
        manifests
            .into_iter()
            .filter(|m| {
                m.targets.is_empty() 
                || m.targets.contains(&ManifestTarget::Any)
                || (boot_target == BootTarget::Terminal && m.targets.contains(&ManifestTarget::Terminal))
                || (boot_target == BootTarget::Gui && m.targets.contains(&ManifestTarget::Gui))
            })
            .map(|m| Service {
                state: if m.enabled {ServiceState::Offline} else {ServiceState::Disabled},
                manifest: m,
            })
            .collect()
    } 
    else {
        // Fallback: Wenn keine Manifest-Dateien angelegt wurden in /loader/initrd/services, wird das Terminal oder die GUI-Services basierend auf dem Boot-Target gestartet
        let terminal_enabled = boot_target == BootTarget::Terminal;
        let gui_enabled = boot_target == BootTarget::Gui;

        let mut out: Vec<Service> = Vec::new();
        out.push(Service {
            state: if terminal_enabled { ServiceState::Offline } else { ServiceState::Disabled },
            manifest: ServiceManifest {
                name: "terminal".to_string(),
                binary: "terminal_emulator".to_string(),
                args: Vec::new(),
                enabled: terminal_enabled,
                targets: alloc::vec![ManifestTarget::Any],
                restart: RestartPolicy::OnFailure,
                heartbeat: false,
                require_all: Vec::new(),
                require_any: Vec::new(),
                optional_all: Vec::new(),
                exclude_all: Vec::new(),
            },
        });
        out.push(Service {
            state: if gui_enabled { ServiceState::Offline } else { ServiceState::Disabled },
            manifest: ServiceManifest {
                name: "window_manager".to_string(),
                binary: "window_manager".to_string(),
                args: Vec::new(),
                enabled: gui_enabled,
                targets: alloc::vec![ManifestTarget::Any],
                restart: RestartPolicy::OnFailure,
                heartbeat: false,
                require_all: Vec::new(),
                require_any: Vec::new(),
                optional_all: Vec::new(),
                exclude_all: Vec::new(),
            },
        });
        out
    };

    let service_states = init_service_status_map(&services);

    let heartbeats: Arc<Mutex<HeartbeatMap>> = Arc::new(Mutex::new(BTreeMap::new()));
    let any_heartbeat = services.iter().any(|s| s.manifest.enabled && s.manifest.heartbeat);
    if any_heartbeat {
        let _ = naming::mkdir(HEARTBEAT_DIR);
        start_watchdog(heartbeats.clone(), service_states.clone());
        //println!("svc: watchdog enabled (dir={}, interval={}ms, timeout={}ms)", HEARTBEAT_DIR, HEARTBEAT_INTERVAL_MS, HEARTBEAT_TIMEOUT_MS);
    }

    if services.is_empty() {
        println!("service_manager: nothing enabled for this boot target; dropping to idle loop");
    } 
    else {
        start_services_with_dependencies(&mut services, &heartbeats, &service_states);
    }

    loop {
        thread::sleep(5_000);
    }
}
