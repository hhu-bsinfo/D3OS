use core::arch::asm;
use log::{error, info, warn};
use naming::shared_types::OpenOptions;
use crate::capabilities::capability::{Capability, CapabilityFlags};
use crate::{process_manager, scheduler, PROCESS_MANAGER};
use crate::capabilities::capability_objects::naming_object::NamingObject;

/**
Share cap with same permissions
 */
pub extern "sysv64" fn sys_share_syscall_cap(thread_id: usize, syscall_number: usize) -> isize {
    let cur_thread = scheduler().current_thread();
    let shared_cap =
        if let Some(sharer_cspace) = cur_thread.cspace.invoke(){
            if let Some(syscall_cap) = sharer_cspace.get_syscall_capability(syscall_number) {
                syscall_cap.share(syscall_cap.get_permissions())
            } else {
                error!(" sharing naming cap: naming cap not found in sharer cspace");
                None
            }
        } else {
            error!(" sharing naming cap: failed to invoke sharer cspace");
            None
        };

    if let Some(receiver_thread) = scheduler().thread(thread_id) {
        if let Some(mut cspace) = receiver_thread.cspace.invoke() {
            // info!("     cspace found");
            if let Some(ref cap) = shared_cap {
                return cspace.receive_syscall_capability(shared_cap, syscall_number);
            }
        } else {
            error!(" receiver cspace not found")
        }
    }
    -5
}
pub extern "sysv64" fn sys_revoke_syscall_cap(thread_id: usize, syscall_number: usize) -> isize {
    if let Some(thread) = scheduler().thread(thread_id){
        if let Some(mut cspace) = thread.cspace.invoke(){
            //todo go through all cspaces and look for shares -> revoke them
            cspace.revoke_syscall_capability(syscall_number);
            return 0;
        }
    }
    -5
}

pub extern "sysv64" fn sys_share_naming_cap(thread_id: usize, rights: usize, naming_object_number: usize) -> isize {
    let rights = OpenOptions::from_bits(rights).unwrap_or_else(|| { OpenOptions::empty() });
    let flags = { 
        let mut flags = CapabilityFlags::empty();
        if rights.contains(OpenOptions::READONLY) || rights.contains(OpenOptions::READWRITE) {
            flags |= CapabilityFlags::READ;
        }
        if rights.contains(OpenOptions::WRITEONLY) || rights.contains(OpenOptions::READWRITE) {
            flags |= CapabilityFlags::WRITE;
        }
        if rights.contains(OpenOptions::CREATE) {
            flags |= CapabilityFlags::EXECUTE;
        }
        if rights.contains(OpenOptions::SHARE) {
            flags |= CapabilityFlags::SHARE;
        }
        flags
    };
    
    info!( "sys_share_naming_cap: called with thread_id {}, rights {}, naming_object_number {}", thread_id, rights.bits(), naming_object_number);
    let cur_thread = scheduler().current_thread();
    // info!(" sharing naming cap: started");
    
    // Scope the first lock so it's dropped before we try to acquire the second one
    let shared_cap = 
        if let Some(sharer_cspace) = cur_thread.cspace.invoke(){
            if let Some(naming_cap) = sharer_cspace.get_naming_capability(naming_object_number) {
                // info!(" sharing naming cap: found naming cap in sharer cspace");
                if naming_cap.is_none() { warn!( "sharing naming cap: naming cap is none") }
                let perms = naming_cap.get_permissions().intersection(flags);
                naming_cap.share(perms)
            } else {
                error!(" sharing naming cap: naming cap not found in sharer cspace");
                return -5;
            }
        } else {
            error!(" sharing naming cap: failed to invoke sharer cspace");
            return -5;
    };

    if let Some(receiver_thread) = scheduler().thread(thread_id) {
        // info!(" sharing naming cap: found receiver thread");
        if let Some(mut cspace) = receiver_thread.cspace.invoke() {
            // info!("     cspace found");
            if let Some(ref cap) = shared_cap {
                let receiver_handle = cspace.receive_naming_capability(shared_cap);
                // info!("     naming cap shared, receiver handle: {}", cspace.get_naming_capabilities_len());
                return receiver_handle;
            }
        }
    } else {
        error!(" sharing naming cap: receiver thread not found")
    }
    -1
}

pub extern "sysv64" fn sys_naming_len() -> usize {
    let cur_thread = scheduler().current_thread();
    if let Some(cspace) = cur_thread.cspace.invoke(){
        return cspace.get_naming_capabilities_len();
    } else {
        0
    }
}

///revokes a shared naming capability from a thread's cspace completely
/// To revoke from other threats and oneself: first revoke from all shares
pub extern "sysv64" fn sys_revoke_naming_cap(thread_id: usize, naming_object_number: usize) -> isize {
    info!("sys_revoke_naming_cap: called with thread_id {}, naming_object_number {}", thread_id, naming_object_number);
    let current_thread = scheduler().current_thread();
    let current_process = scheduler().current_ids().0;
    let Some(mut cspace) = current_thread.cspace.invoke() else { return -5 };
    let Some(cap_to_revoke) = cspace.get_naming_capability_mut(naming_object_number) else { return -5 };
    
    if thread_id == current_thread.id() {
        cap_to_revoke.revoke();
        0
    } else {
        let Some(revoke_thread) = scheduler().thread(thread_id) else { return -4; };
        let Some(mut revoke_cspace) = revoke_thread.cspace.invoke() else { return -3; };
        let Ok(cap_handle) = revoke_cspace.cap_with_same_obj(cap_to_revoke) else { return -2; };
        let Some(cap_to_revoke) = revoke_cspace.get_naming_capability_mut(cap_handle) else { return -1; };
        
        // let processes = process_manager().read().active_process_ids();
        // for i in processes { //go through all cspaces and look for shares -> revoke them
        //     if i == current_process { continue; } //skip current process as revoke from self not wanted
        //     let process = process_manager().read().process(i);
        //     let res = process.cspace.invoke();
        //     match res {
        //         Some(mut cspace) => {
        //             cspace.revoke_naming_capability(cap_to_revoke);
        //         },
        //         None => {}//will fail to open current's process cspace. Ok as revoke from self not wanted here
        //     }
        // }
        if !cap_to_revoke.is_original() { cap_to_revoke.revoke();} //Revoke cap from other thread. originial can only be revoked from owner thread
        0
    }
}

///revokes specific rights from a shared naming capability from a thread's cspace
pub extern "sysv64" fn sys_revoke_naming_rights(thread_id: usize, naming_object_number: usize, rights: usize) -> isize {
    let rights_to_revoke = CapabilityFlags::from_bits(rights as u32).unwrap_or_else(|| { CapabilityFlags::empty() });
    let current_thread = scheduler().current_thread();
    let current_process = scheduler().current_ids().0;
    let Some(mut cspace) = current_thread.cspace.invoke() else { return -5 };
    let Some(cap) =  cspace.get_naming_capability(naming_object_number) else { return -5 };


    let Some(mut cap_to_revoke) = cspace.get_naming_capability_mut(naming_object_number) else { return -5 };
    let processes = process_manager().read().active_process_ids();

    for i in processes{ //go through all cspaces and look for shares -> revoke rights
        if i == current_process { continue; } //skip current process as revoke from self not wanted
        let process = process_manager().read().process(i);
        let res = process.cspace.invoke();
        match res {
            Some(mut cspace) => {
                cspace.revoke_naming_rights(cap_to_revoke, rights_to_revoke);},
            None => {}} //will fail to open current's process cspace. Ok as revoke from self not wanted here
    }
    cap_to_revoke.revoke_rights(rights_to_revoke);
    0isize
}