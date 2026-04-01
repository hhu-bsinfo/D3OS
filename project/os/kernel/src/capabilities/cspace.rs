#![warn(missing_docs)]

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::arch::x86_64::__get_cpuid_max;
use core::ops::{Add, Deref};
use log::{info, warn};
use spin::Once;
use naming::shared_types::OpenOptions;
use syscall::NUM_SYSCALLS;
use syscall::return_vals::Errno;
use crate::capabilities::capability;
use crate::capabilities::capability::{Capability, CapabilityFlags};
use crate::capabilities::capability_objects::naming_object::{create_naming_capability, NamingObject};
use crate::capabilities::capability_objects::syscall_object::Syscall;
use crate::device::cpu;
use crate::naming::{api, lookup};
use crate::naming::api::shared_pipe;
use crate::naming::traits::{as_named_object, DirectoryObject, NamedObject};
use crate::syscall::sys_concurrent::*;
use crate::syscall::sys_naming::*;
use crate::syscall::sys_terminal::*;
use crate::syscall::sys_time::*;
use crate::syscall::sys_vmem::*;
use crate::syscall::sys_caps::*;
use crate::syscall::sys_net::*;

const BROADCAST_PIPE : Once<Capability<NamingObject>> = Once::new();

pub struct CSpace{
    syscall_capabilities: Vec<Capability<Syscall>>,
    naming_capabilities: Vec<Capability<NamingObject>>,
    open_naming_capabilities: Vec<Capability<NamingObject>>, //caps that point to objects that are currently open, used for read/write
    //... other capability types
}

impl CSpace{ 
    /// Create a new CSpace with all capabilities initialized to the default values
    pub fn new() -> Self {
        let syscall_fns: [*const (); NUM_SYSCALLS] = [
            sys_terminal_read as *const (), //0
            sys_terminal_read_nb as *const (),
            sys_terminal_write as *const (),
            sys_map_memory as *const (),
            sys_map_frame_buffer as *const (),
            sys_process_execute_binary as *const (), //5
            sys_process_id as *const (),
            sys_process_exit as *const (),
            sys_thread_create as *const (),
            sys_thread_id as *const (),
            sys_thread_switch as *const (), //10
            sys_thread_sleep as *const (),
            sys_thread_join as *const (),
            sys_thread_exit as *const (),
            sys_get_system_time as *const (),
            sys_get_date as *const (), //15
            sys_set_date as *const (),
            sys_open as *const (),
            sys_read as *const (),
            sys_write as *const (),
            sys_seek as *const (), //20
            sys_close as *const (),
            sys_mkdir as *const (),
            sys_touch as *const (),
            sys_readdir as *const (),
            sys_cwd as *const (), //25
            sys_cd as *const (),
            sys_sock_open as *const (),
            sys_sock_bind as *const (),
            sys_sock_accept as *const (),
            sys_sock_connect as *const (), //30
            sys_sock_send as *const (),
            sys_sock_receive as *const (),
            sys_sock_close as *const (),
            sys_get_ip_adresses as *const (),
            sys_mkfifo as *const (), //35
            //caps
            sys_share_syscall_cap as *const (),
            sys_revoke_syscall_cap as *const (),
            sys_share_naming_cap as *const (),
            sys_revoke_naming_cap as *const (),
            sys_naming_len as *const (), //40
        ]; 
        
        let mut num = 0;
        let mut syscall_capabilities: Vec<_> = syscall_fns
            .iter()
            .map(|&f| {
                let cap = Capability::syscall(Syscall::new(num, f));
                num += 1;
                cap
            })
            .collect();

             // Example of revoking a specific syscall capability
         if let Some(mut cap) = syscall_capabilities.get_mut(13) {
            //cap.revoke();
        }

        let mut naming_capabilities = Vec::new();
        //check if naming is initialized already
        if api::ROOT.is_completed() {
            if let Some(root) = api::ROOT.get(){
                let root_cap = api::root();
                let shared_pipe = shared_pipe(&root_cap);
                naming_capabilities.push(root_cap); //ROOT at index 0
                naming_capabilities.push(shared_pipe); //SHARED_PIPE at index 1
            }
            // naming_capabilities.push(api::root());
        }


        
        // if let Some(root) = api::ROOT.get(){ 
        //     let root_cap = create_naming_capability(NamedObject::from(root.root_dir()), OpenOptions::all(), None);//NamedObject::DirectoryObject(root.root_dir()), OpenOptions::all(), None);
        //     naming_capabilities.push(root_cap);
        // }
        
        Self {
            syscall_capabilities,
            naming_capabilities,
            open_naming_capabilities: Vec::new(),
            //memory_capabilities: Vec::new(),
            //driver_capabilities: Vec::new(),
            //... initialize other capability types
        }
    }
    
    ///Saves the provided syscall capability to the CSpace, returns the index of the capability in the CSpace
    pub fn receive_syscall_capability(&mut self, capability: Option<Capability<Syscall>>, syscall_num: usize) -> isize{
        if let Some(capability) = capability {
            if let Some(cap) = self.syscall_capabilities.get_mut(syscall_num) {
                if let Some(combined) = capability.combine(cap){
                    self.syscall_capabilities[syscall_num] = combined
                } //else they dont point to the same syscall so keep current
            } else {
                self.syscall_capabilities[syscall_num] = capability;
            }
            return syscall_num.try_into().unwrap(); //panics if syscall num > isize::MAX (9_223_372_036_854_775_808) --> practically impossible
        }
        -1
    }
    
    ///Revoke the provided syscall capability from the CSpace
    pub fn revoke_syscall_capability(&mut self, syscall_num: usize){
        if let Some(cap) = self.syscall_capabilities.get_mut(syscall_num) {

            cap.revoke();
        }
    }
    
    ///Returns the syscall capability at the provided index in the CSpace, if it exists
    pub fn get_syscall_capability(&self, syscall_num: usize) -> Option<&Capability<Syscall>> {
        self.syscall_capabilities.get(syscall_num)
    }
    
    ///Returns the syscall capability at the provided index in the CSpace, if it exists
    pub fn get_syscall_capability_mut(&mut self, syscall_num: usize) -> Option<&mut Capability<Syscall>> {
        self.syscall_capabilities.get_mut(syscall_num)
    }
    
    ///Removes the syscall capability at the provided index in the CSpace, if it exists
    pub fn remove_syscall_capability(&mut self, syscall_num: usize) -> Capability<Syscall> {
        self.syscall_capabilities.remove(syscall_num)
    }

    ///Saves the provided naming capability to the CSpace, returns the index of the capability in the CSpace
    pub(crate) fn receive_root_naming_capability(&mut self, capability: Option<Capability<NamingObject>>) -> isize{
        if let Some(cap) = capability {
            self.naming_capabilities[0] = cap;
            return 0; //panic if len > isize::MAX (9_223_372_036_854_775_808) --> practically impossible
        }

        -1
    }

    ///Saves the provided naming capability to the CSpace, returns the index of the capability in the CSpace
    pub fn receive_naming_capability(&mut self, capability: Option<Capability<NamingObject>>) -> isize{
        if let Some(cap) = capability {
            // info!("     CSpace: Naming capability is none: {}", cap.is_none());
            self.naming_capabilities.push(cap);
            // info!("     CSpace: Received naming capability, new length {}", self.naming_capabilities.len());
            return self.naming_capabilities.len() as isize - 1; //panic if len > isize::MAX (9_223_372_036_854_775_808) --> practically impossible
        }

        warn!("     CSpace: Failed to receive naming capability");
        -1
    }
    
    ///Saves the provided naming capability to the CSpace, returns the index of the capability in the CSpace
    pub fn receive_open_naming_capability(&mut self, capability: Option<Capability<NamingObject>>) -> isize{
        if let Some(cap) = capability {
            // info!("     CSpace: Naming capability is none: {}", cap.is_none());
            self.open_naming_capabilities.push(cap);
            // info!("     CSpace: Received naming capability, new length {}", self.open_naming_capabilities.len());
            return self.open_naming_capabilities.len() as isize - 1; //panic if len > isize::MAX (9_223_372_036_854_775_808) --> practically impossible
        }

        warn!("     CSpace: Failed to receive naming capability");
        -1
    }

    ///returns the opened naming capability at the provided index in the CSpace, if it exists
    pub fn get_open_naming_capability(&self, handle: usize) -> Option<&Capability<NamingObject>> {
        self.open_naming_capabilities.get(handle)
    }

    ///returns the opened naming capability at the provided index in the CSpace, if it exists
    pub fn get_open_naming_capability_mut(&mut self, handle: usize) -> Option<&mut Capability<NamingObject>> {
        self.open_naming_capabilities.get_mut(handle)
    }

    ///returns the naming capability at the provided index in the CSpace, if it exists
    pub fn get_naming_capability(&self, handle: usize) -> Option<&Capability<NamingObject>> {
        self.naming_capabilities.get(handle)
    }

    ///returns the naming capability at the provided index in the CSpace, if it exists
    pub fn get_naming_capability_mut(&mut self, handle: usize) -> Option<&mut Capability<NamingObject>> {
        self.naming_capabilities.get_mut(handle)
    }

    ///returns the number of naming capabilities in the CSpace
    pub fn get_naming_capabilities_len(&self) -> usize {
        self.naming_capabilities.len()
    }

    ///returns the number of open naming capabilities in the CSpace
    pub fn get_open_naming_capabilities_len(&self) -> usize {
        self.open_naming_capabilities.len()
    }

    ///Removes the naming capability at the provided index in the CSpace, if it exists
    pub fn close_open_naming_capability(&mut self, handle: usize) -> isize{
        if let Some(cap) = self.open_naming_capabilities.get_mut(handle) {
            cap.revoke();
            self.open_naming_capabilities.remove(handle);
            return 0;
        }
        warn!("     CSpace: Failed to close open naming capability, capability not found");
        -1
    }
    //
    // pub fn debug_print_caps(&self){
    //     info!("CSpace: Dumping syscall capabilities:");
    //     for (i, cap) in self.syscall_capabilities.iter().enumerate(){
    //         info!("    Syscall {}: {:?}", i, cap);
    //     }
    //     info!("CSpace: Dumping naming capabilities:");
    //     for (i, cap) in self.naming_capabilities.iter().enumerate(){
    //         info!("    Naming Cap {}: {:?}", i, cap);
    //     }
    // }
    
    /// check if the object from the provided capability is stored and if it was shared by the provided capability, if yes then revoke the cap, otherwise do nothing
    pub fn revoke_naming_capability(&mut self, cap: &Capability<NamingObject>) -> Result<isize, Errno>{
        let mut handle = -1isize;
        let was_enabled = cpu::disable_int_nested();
        let mut capability_revoked = false;

        // Iterate through all naming capabilities and check for matches
        for i in 0..self.naming_capabilities.len() {
            let capability = &mut self.naming_capabilities[i];
            info!(
                "     CSpace: Checking naming capability {}, {}",
                i,
                capability.points_to_same_object(cap)
            );
            if capability.points_to_same_object(cap) {
                if cap.was_shared_to(capability) {
                    info!("                 shared");
                    handle = i as isize;
                    capability.revoke();
                    capability_revoked = true;
                } else {
                    warn!("                 not shared");
                }
            }
        }

        for capability in self.open_naming_capabilities.iter_mut() {
            info!("     CSpace: Checking open naming capability for revoke, cap points to same object: {}", capability.points_to_same_object(cap));
            if capability.points_to_same_object(cap) {
                warn!("                 open");
                capability.revoke();
                capability_revoked = true;
            }
        }
        cpu::enable_int_nested(was_enabled);
        if capability_revoked {
            info!("     CSpace: Successfully revoked matching naming capabilities");
            Ok(handle)
        } else {
            warn!("     CSpace: Failed to revoke naming capability, no matching capability found");
            Err(Errno::EUNKN)
        }
    }

    ///Same checks as with revoke_naming_capability but only revokes the provided rights instead of the whole cap
    pub fn revoke_naming_rights(&mut self, cap: &Capability<NamingObject>, rights: CapabilityFlags) -> isize{
        let was_enabled = cpu::disable_int_nested();
        if rights.is_empty() {
            warn!("     CSpace: Failed to revoke naming rights, no rights provided");
            return -1;
        }

        let mut rights_revoked = false;


        // Iterate over all naming capabilities and find all that match the object of the provided cap
        for capability in self.naming_capabilities.iter_mut() {
            if capability.points_to_same_object(cap) {
                if cap.was_shared_to(capability) {
                    capability.revoke_rights(rights);
                    rights_revoked = true;
                }
            }
        }

        for capability in self.open_naming_capabilities.iter_mut() {
            if capability.points_to_same_object(cap) {
                if cap.was_shared_to(capability) {
                    capability.revoke_rights(rights);
                    rights_revoked = true;
                }
            }
        }

        cpu::enable_int_nested(was_enabled);
        if rights_revoked {
            return 0isize;
        }
        -1
    }
    
    ///Returns the index of the capability pointing to the same object as the provided capability, if it exists
    pub(crate) fn cap_with_same_obj(&self, cap: &Capability<NamingObject>) -> Result<usize, Errno>{
        for (i, capability) in self.naming_capabilities.iter().enumerate() {
            if capability.points_to_same_object(cap) {
                return Ok(i);
            }
        }
        Err(Errno::EUNKN)
    }
    
}