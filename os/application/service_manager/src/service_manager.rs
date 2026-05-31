#![no_std]
#[allow(unused_imports)]
extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use concurrent::thread;
use naming::shared_types::{FileType, OpenOptions};
use syscall::return_vals::Errno;
use runtime::*;
use serde::Deserialize;
use terminal::println;
use log::error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BootTarget {   // Um zu wissen welcher Service gestartet werden soll
    Terminal,
    Gui,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Targets(u8);

impl Targets {
    const TERMINAL: u8 = 1 << 0;
    const GUI: u8 = 1 << 1;
    const ANY: u8 = Self::TERMINAL | Self::GUI;

    const fn none() -> Self {
        Self(0)
    }

    const fn any() -> Self {
        Self(Self::ANY)
    }

    const fn has(self, bit: u8) -> bool {
        (self.0 & bit) != 0
    }

    const fn matches_boot_target(self, target: BootTarget) -> bool {
        match target {
            BootTarget::Terminal => self.has(Self::TERMINAL),
            BootTarget::Gui => self.has(Self::GUI),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceState {     // Alle möglichen Zustände eines Service
    Uninitialized,
    Disabled,
    Offline,
    Online,
    Degraded,
    Maintenance,
}

fn is_running_state(state: ServiceState) -> bool {
    matches!(state, ServiceState::Online | ServiceState::Degraded)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")] // Automatisch in snake_case umwandeln fürs leichtere parsen
enum RestartPolicy {    // Wie umgehen mit Service-Ausfällen?
    Never,
    Always,
    OnFailure,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum RestartOn {    // steht im paper, aber ich verstehe den Zweck noch nicht ganz
    None,
    Error,
    Restart,
    Refresh,
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

fn default_restart_policy() -> RestartPolicy {
    RestartPolicy::OnFailure
}

fn default_restart_on() -> RestartOn {
    RestartOn::Restart
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

    #[serde(default = "default_restart_on")]
    restart_on: RestartOn,

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
        let mut target_bits = Targets::none();

        if self.targets.is_empty() {
            target_bits = Targets::any();
        } 
        else {
            for t in &self.targets {
                match t {
                    ManifestTarget::Terminal => target_bits.0 |= Targets::TERMINAL,
                    ManifestTarget::Gui => target_bits.0 |= Targets::GUI,
                    ManifestTarget::Any => {
                        target_bits = Targets::any();
                        break;
                    }
                }
            }

            if target_bits.0 == 0 {
                target_bits = Targets::any();
            }
        }

        ServiceManifest {
            name: self.name,
            binary: self.binary,
            args: self.args,
            enabled: self.enabled,
            targets: target_bits,
            restart: self.restart,
            restart_on: self.restart_on,
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
    targets: Targets,
    restart: RestartPolicy,
    restart_on: RestartOn,
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

fn should_restart(policy: RestartPolicy, exit_code: usize) -> bool {
    match policy {
        RestartPolicy::Never => false,
        RestartPolicy::Always => true,
        RestartPolicy::OnFailure => exit_code != 0,
    }
}

fn supervise_service(manifest: ServiceManifest, mut child: thread::Thread) {
    loop {
        let exit_code = match child.join() {
            Ok(code) => code,
            Err(e) => {
                println!("svc: {} join failed (err={:?})", manifest.name, e);
                1
            }
        };

        println!("svc: {} exited (code={})", manifest.name, exit_code);

        if !should_restart(manifest.restart, exit_code) {
            println!("svc: {} not restarting (policy={:?})", manifest.name, manifest.restart);
            break;
        }

        thread::sleep(200); // Kurz Verzögern, um Crash-Loops zu vermeiden

        println!("svc: restarting {} -> {}", manifest.name, manifest.binary);
        let args_vec: Vec<&str> = manifest.args.iter().map(|s| s.as_str()).collect();
        match thread::start_application(&manifest.binary, args_vec) {
            Some(new_child) => {
                println!("svc: {} online (tid={})", manifest.name, new_child.id());
                child = new_child;
            }
            None => {
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
// - require_all: erst starten, wenn alle davon laufen
// - require_any: erst starten, wenn mindestens einer davon läuft (falls die Liste nicht leer ist)
// - optional_all: wie require_all, aber nur falls der Service überhaupt existiert; fehlt er, wird er ignoriert
// Alle Dienste in require_all müssen im Online oder Degraded Zustand sein, damit der Service gestartet werden kann.
fn can_start_now(service: &Service, services: &[Service]) -> bool {
    // exclude_all: don't start while excluded services are running.
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
        if !is_running_state(dep_state) {
            return false;
        }
    }

    true
}

fn start_service_once(service: &mut Service) -> bool {
    println!("svc: start {} -> {}", service.manifest.name, service.manifest.binary);
    let args_vec: Vec<&str> = service.manifest.args.iter().map(|s| s.as_str()).collect();

    match thread::start_application(&service.manifest.binary, args_vec) {
        Some(child) => {
            let manifest = service.manifest.clone();

            service.state = ServiceState::Online;
            println!("svc: {} online (tid={})", service.manifest.name, child.id());

            let _ = thread::create(move || supervise_service(manifest, child));
            true
        }
        None => {
            service.state = ServiceState::Maintenance;
            println!("svc: {} entered maintenance (exec failed)", service.manifest.name);
            false
        }
    }
}

// Geht über alle Services und starte alle die, die dürfen (Offline + Abhängigkeiten erfüllt)
fn start_eligible_services(services: &mut [Service], start_order: &mut Vec<String>) -> usize {
    let mut started = 0;
    for i in 0..services.len() {
        if services[i].state != ServiceState::Offline {
            continue;
        }

        if !can_start_now(&services[i], services) {
            continue;
        }

        let ok = start_service_once(&mut services[i]);
        if ok {
            start_order.push(services[i].manifest.name.clone());
        }
        started += 1;
    }
    started
}

// Schleife, bis alle Services gestartet wurden. 
fn start_services_with_dependencies(services: &mut [Service]) {
    let mut start_order: Vec<String> = Vec::new();

    loop {
        let started_now = start_eligible_services(services, &mut start_order);
        if started_now == 0 {   // Nichts mehr gestaret, also entweder alle online oder Zyklus
            break;
        }
    }

    if !start_order.is_empty() {
        println!("svc: start order ({}):", start_order.len());
        for name in &start_order {
            println!("svc:   {}", name);
        }
    }

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
fn print_manifest(m: &ServiceManifest) {
    println!("svc: manifest name={} binary={} enabled={} restart={:?} restart_on={:?}",
        m.name, m.binary, m.enabled, m.restart, m.restart_on);
    println!("svc:   targets_bits=0x{:x} args={} require_all={} require_any={} optional_all={} exclude_all={}",
        m.targets.0,
        m.args.len(),
        m.require_all.len(),
        m.require_any.len(),
        m.optional_all.len(),
        m.exclude_all.len(),
    );
}

fn parse_boot_target() -> BootTarget {
    let mut args = env::args();
    let _ = args.next();

    for arg in args {
        match arg.as_str() {
            "--gui" => return BootTarget::Gui,
            "--terminal" => return BootTarget::Terminal,
            "--help" | "-h" => {
                println!("Usage: service_manager [--gui|--terminal]");
                return BootTarget::Terminal;
            }
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

    println!("D3OS service_manager starting...");

    let boot_target = parse_boot_target();
    println!("service_manager: boot_target={:?}", boot_target);

    let manifests = load_service_manifests();
    println!("svc: loaded {} manifest(s)", manifests.len());
    //for m in &manifests {
        //print_manifest(m);
    //}

    let mut services: Vec<Service> = if !manifests.is_empty() {
        manifests
            .into_iter()
            .filter(|m| m.enabled && m.targets.matches_boot_target(boot_target))
            .map(|m| Service {
                state: ServiceState::Offline,
                manifest: m,
            })
            .collect()
    } 
    else {
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
                targets: Targets::any(),
                restart: RestartPolicy::OnFailure,
                restart_on: RestartOn::Restart,
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
                targets: Targets::any(),
                restart: RestartPolicy::OnFailure,
                restart_on: RestartOn::Restart,
                require_all: Vec::new(),
                require_any: Vec::new(),
                optional_all: Vec::new(),
                exclude_all: Vec::new(),
            },
        });
        out
    };

    if services.is_empty() {
        println!("service_manager: nothing enabled for this boot target; dropping to idle loop");
    } 
    else {
        start_services_with_dependencies(&mut services);
    }

    loop {
        thread::sleep(5_000);
    }
}
