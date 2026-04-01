#![warn(missing_docs)]

use alloc::sync::Weak;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::bitflags;
use log::{error, info, warn};
use pc_keyboard::KeyCode::Mute;
use spin::{Mutex, MutexGuard};
use crate::capabilities::capability_objects::naming_object::NamingObject;

/// Flags to describe permissions associated with capabilities.
///
/// The flags are defined using the `bitflags` crate, and include:
/// - `READ`: The resource can be read.
/// - `WRITE`: The resource can be modified.
/// - `EXECUTE`: The resource can be executed.
/// - `SHARE`: The resource can be shared.

bitflags! {
    #[derive(Clone, Copy)]
    pub struct CapabilityFlags: u32 {
        const READ =     0b00000001;
        const WRITE =    0b00000010;
        const EXECUTE =  0b00000100;
        const SHARE =    0b00001000;
    }
}

pub struct Capability<T> {
    obj: Option<Arc<Mutex<T>>>,
    flags: CapabilityFlags,
    shared_to: Mutex<Vec<Weak<Capability<T>>>>, // Reference to the capability that shared this one
    original: bool,
}

impl<T> Capability<T> {
    
    ///Returns true if the capability is None, typically because it has been revoked.
    pub(crate) fn is_none(&self) -> bool {
        self.obj.is_none()
    }
}

impl<T> Capability<T> {

    /// Creates a new, original `Capability` with the given resource and permissions.
    pub fn new(obj: T, flags: CapabilityFlags) -> Self {
        Self {
            obj: Some(Arc::new(Mutex::new(obj))),
            flags,
            shared_to: Mutex::new(Vec::new()), 
            original: true
        }
    }

    ///returns true if the capability has the specified permissions
    pub fn has_permissions(&self, flags: CapabilityFlags) -> bool {
        self.flags.contains(flags)
    }

    ///returns the permissions of the capability
    pub fn get_permissions(&self) -> CapabilityFlags {
        self.flags
    }
    
    ///returns true if the capability is original, i.e. it was not shared from another capability
    pub fn is_original(&self) -> bool {
        self.original
    }
    
    ///invokes the capability, i.e. locks the underlying resource and returns a guard to it
    pub fn invoke(&self) -> Option<MutexGuard<'_, T>> {
        if !self.flags.intersects(CapabilityFlags::READ | CapabilityFlags::WRITE | CapabilityFlags::EXECUTE) { 
            warn!("Tried to invoke a capability without READ permission");
            return None;
        }
        self.obj.as_ref()?.try_lock()
    }

    /// Shares this capability, creating a new capability with the specified permissions
    /// Sharing is only allowed if the original capability has the SHARE permission
    ///
    /// It should only be used if absolutely necessary *and* if a revoke is unlikely to be needed
    pub fn share(&self, new_flags: CapabilityFlags) -> Option<Capability<T>> {
        if !self.has_permissions(CapabilityFlags::SHARE) {
            return None;
        }

        self.obj.as_ref().map(|arc| {
            let new_cap = Capability {
                obj: Some(Arc::clone(arc)),
                flags: new_flags & self.flags,
                shared_to: Mutex::new(Vec::new()),
                original: false
            };

            // Store weak reference to the new capability
            let new_cap_arc : Arc<Capability<T>> = Arc::new(new_cap.clone());
            self.shared_to.try_lock().unwrap().push(Arc::downgrade(&new_cap_arc));
    
            info!("Shared cap");
            new_cap
        })
    }

    //Transfer via Share + Revoke on self as transfer logically is rarely used -> single method of sharing
    // pub fn transfer(&mut self, new_flags: CapabilityFlags) -> Option<Capability<T>> {
    //     if !self.has_permissions(CapabilityFlags::TRANSFER) {
    //         return None;
    //     }
    //
    //     if !self.flags.contains(new_flags) {
    //         return None;
    //     }
    //
    //     let new_cap = self.obj.as_ref().map(|arc| Capability {
    //         obj: Some(Arc::clone(arc)),
    //         flags: new_flags,
    //     });
    //
    //     if new_cap.is_some() {
    //         self.revoke();
    //     }
    //
    //     new_cap
    // }

    /// Revokes this capability, making it unusable
    ///
    /// Revoke should rarely be necessary as capabilities should only be shared if absolutely needed
    pub fn revoke(&mut self) {
        self.obj = None;
        self.flags = CapabilityFlags::empty();
        // Note: shared_by remains to maintain the revocation chain
    }

    /// Revokes this capability's rights
    /// 
    /// Revoke should rarely be necessary as capabilities should only be shared if absolutely needed
    pub fn revoke_rights(&mut self, rights: CapabilityFlags) {
        self.flags = self.flags - rights;
    }

    
    /// Returns true if this capability has been shared to the specified capability
    pub fn was_shared_to(&self, other: &Capability<T>) -> bool {
        let Some(shared_to) = self.shared_to.try_lock() else {
            warn!("Could not acquire lock on shared_to list.");
            return false;
        };

        shared_to.iter().any(|weak_cap| {
            weak_cap.as_ptr() == Arc::as_ptr(&Arc::new(other.clone()))
        })
    }

    ///Combines two capabilities into one, merging their permissions and shared_to lists if they refer to the same object
    pub(crate) fn combine(&self, other: &Capability<T>) -> Option<Capability<T>> {
        // Only allow combining if both capabilities refer to the same object
        if let Some(obj) = &self.obj {
            if let Some(other_obj) = &other.obj {
                if Arc::<Mutex<T>>::as_ptr(obj) == Arc::<Mutex<T>>::as_ptr(other_obj) {
                    let mut merged_shared_to = self.shared_to.try_lock().unwrap().clone();
                    merged_shared_to.extend(other.shared_to.try_lock().unwrap().iter().cloned());

                    return Some(Capability {
                        obj: self.obj.clone(),
                        flags: self.flags.clone() | other.flags.clone(),
                        shared_to: Mutex::new(merged_shared_to), //if combining original capability's lineage is kept
                        original: self.original | other.original, //if one of them is original the combined cap is also original
                    });
                }
            }
        }
        None
    }
    
    ///Returns true if both capabilities refer to the same object
    pub(crate) fn points_to_same_object(&self, other: &Capability<T>) -> bool {
        if let (Some(obj), Some(other_obj)) = (&self.obj, &other.obj) {
            Arc::as_ptr(obj) == Arc::as_ptr(other_obj)
        } else {
            false
        }
    }

    // pub fn add_permissions(&mut self, flags: CapabilityFlags) {
    //     self.flags.insert(flags);
    // }
    // FORBIDDEN. Get rights by share or transfer!!!
    // Could lead to rights escalation.

}

// Hilfreiche Methoden für die Erstellung von Capabilities mit verschiedenen Berechtigungen
impl<T> Capability<T> {
    pub fn readonly(obj: T) -> Self {
        Self::new(obj, CapabilityFlags::READ)
    }

    pub fn readwrite(obj: T) -> Self {
        Self::new(obj, CapabilityFlags::READ | CapabilityFlags::WRITE)
    }

    pub fn shareable(obj: T) -> Self {
        Self::new(obj, CapabilityFlags::READ | CapabilityFlags::SHARE)
    }

    pub fn full_access(obj: T) -> Self {
        Self::new(obj, CapabilityFlags::all())
    }

    pub fn null() -> Self {
        Self { obj: None, flags: CapabilityFlags::empty(), shared_to: Mutex::new(Vec::new()), original: true }
    }
    
    pub fn syscall(obj: T) -> Self {
        Self::new(obj, CapabilityFlags::EXECUTE | CapabilityFlags::SHARE)
    }
}

impl<T> Clone for Capability<T> { //Only used for the shared_to chain
    fn clone(&self) -> Self {
        Self {
            obj: self.obj.as_ref().map(Arc::clone),
            flags: self.flags,
            shared_to: Mutex::new(self.shared_to.try_lock().unwrap().clone()),
            original: self.original,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.obj = source.obj.as_ref().map(Arc::clone);
        self.flags = source.flags;
        self.shared_to = Mutex::new(source.shared_to.try_lock().unwrap().clone());
        self.original = source.original;
    }
}