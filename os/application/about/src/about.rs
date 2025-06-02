#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use chrono::DateTime;
#[allow(unused_imports)]
use runtime::*;
use serde::{Deserialize, Serialize};
use terminal::{print, println};
use terminal::read::read;

mod built_info {
    // The file has been placed there by the build script.
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Serialize, Deserialize)]
struct Dependency {
    name: Option<String>,
    version: Option<String>,
    authors: Option<String>,
    repository: Option<String>,
    license: Option<String>,
    license_file: Option<String>,
    description: Option<String>,
}

#[unsafe(no_mangle)]
pub fn main() {
    let dependencies_file = include_str!("dependencies.json");
    let dependencies: Vec<Dependency> = serde_json::from_str(dependencies_file).unwrap();

    let git_ref = built_info::GIT_HEAD_REF.unwrap_or("Unknown");
    let git_commit = built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("Unknown");
    let build_date = match DateTime::parse_from_rfc2822(built_info::BUILT_TIME_UTC) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string(),
    };
    
    println!("D3OS v{} ({} - {})", built_info::PKG_VERSION, git_ref, git_commit);
    println!("Build Date: {}", build_date);
    println!("Built by: {} ({} - O{})\n", built_info::RUSTC_VERSION, built_info::PROFILE, built_info::OPT_LEVEL);
    
    println!("Dependencies (Found {}, press ENTER to show next or Q to quit):", dependencies.len());
    
    for dep in dependencies {
        if let Some(name) = dep.name {
            println!("{}", name);
            
            if let Some(version) = dep.version {
                println!("  Version: {}", version);
            }
            
            if let Some(authors) = dep.authors {
                println!("  Authors: {}", authors);
            }
            
            print!("  License: {}", dep.license.unwrap_or_else(|| "Unknown".to_string()));
        }
        
        loop {
            let input = read();
            match input {
                Some('q') | Some('Q') | None => {
                    print!("\n");
                    return; // Exit the application
                },
                Some('\n') => {
                    break; // Proceed to the next dependency
                },
                Some(_) => {} // Ignore any other input
            }
        }
    }
}