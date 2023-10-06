use core::mem;

#[allow(dead_code)]
#[derive(PartialEq, Eq)]
#[repr(u32)]
pub enum TagType {
    Terminate = 0,
    BootCommandLine = 1,
    BootLoaderName = 2,
    Module = 3,
    BasicMemoryInformation = 4,
    BiosBootDevice = 5,
    MemoryMap = 6,
    VbeInfo = 7,
    FramebufferInfo = 8,
    ElfSymbols = 9,
    ApmTable = 10,
    Efi32BitSystemTablePointer = 11,
    Efi64BitSystemTablePointer = 12,
    SmBiosTables = 13,
    AcpiOldRsdp = 14,
    AcpiNewRsdp = 15,
    NetworkingInformation = 16,
    EfiMemoryMap = 17,
    EfiBootServicesNotTerminated = 18,
    Efi32BitImageHandlePointer = 19,
    Efi64BitImageHandlePointer = 20,
    ImageLoadBasePhysicalAddress = 21
}

#[allow(dead_code)]
#[repr(u8)]
pub enum FrameBufferType {
    Indexed = 0,
    Rgb = 1,
    EgaText = 2
}

#[allow(dead_code)]
struct Info {
    size: u32,
    reserved: u32
}

pub struct TagHeader {
    pub tag_type: TagType,
    pub size: u32
}

pub struct FrameBufferInfo {
    pub header: TagHeader,
    pub addr: u64,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
    pub bpp: u8,
}

pub fn get_tag<T>(mbi: u64, tag_type: TagType) -> T {
    let mut addr = mbi + (mem::size_of::<Info>() as u64);
    let mut tag_ptr = addr as *const TagHeader;
    let mut tag = unsafe { tag_ptr.read() };

    while tag.tag_type != TagType::Terminate {
        if tag.tag_type == tag_type {
            unsafe { return (tag_ptr as *const T).read(); }
        }

        addr += tag.size as u64;
        if addr % 8 != 0 {
            addr = (addr / 8) * 8 + 8;
        }

        tag_ptr = addr as *const TagHeader;
        tag = unsafe { tag_ptr.read() };
    }

    panic!("Multiboot: Tag with type [{}] not found!", tag_type as u32);
}