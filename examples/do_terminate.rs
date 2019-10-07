use aeron_rs::client::cnc_descriptor::MetaDataDefinition;
use aeron_rs::client::concurrent::atomic_buffer::AtomicBuffer;
use aeron_rs::client::concurrent::ring_buffer::ManyToOneRingBuffer;
use aeron_rs::client::context::ClientContext;
use aeron_rs::util::IndexT;
use memmap::MmapOptions;
use std::fs::OpenOptions;
use std::mem::size_of;
use aeron_rs::client::cnc_descriptor;

fn main() {
    let path = ClientContext::default_aeron_path();
    let cnc = path.join("cnc.dat");

    println!("Opening CnC file: {}", cnc.display());
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&cnc)
        .expect("Unable to open CnC file");
    let mut mmap =
        unsafe { MmapOptions::default().map_mut(&file) }.expect("Unable to mmap CnC file");
    println!("MMap len: {}", mmap.len());

    // When creating the buffer, we need to offset by the CnC metadata
    let cnc_metadata_len = cnc_descriptor::META_DATA_LENGTH;
    println!("Buffer start: {}", cnc_metadata_len);

    // Read metadata to get buffer length
    let buffer_len = {
        let atomic_buffer = AtomicBuffer::wrap(&mut mmap);
        let metadata = atomic_buffer.overlay::<MetaDataDefinition>(0).unwrap();
        metadata.to_driver_buffer_length
    };
    println!("Buffer len: {}", buffer_len);

    let buffer_end = cnc_metadata_len + buffer_len as usize;
    let atomic_buffer = AtomicBuffer::wrap(&mut mmap[cnc_metadata_len..buffer_end]);
    let mut ring_buffer =
        ManyToOneRingBuffer::wrap(atomic_buffer).expect("Improperly sized buffer");

    // 20 bytes: Client ID (8), correlation ID (8), token length (4)
    let mut terminate_bytes = vec![0u8; 20];
    let terminate_len = terminate_bytes.len();
    let mut source_buffer = AtomicBuffer::wrap(&mut terminate_bytes);
    let client_id = ring_buffer.next_correlation_id();
    source_buffer.put_i64_ordered(0, client_id).unwrap();
    source_buffer.put_i64_ordered(8, -1).unwrap();

    let term_id: i32 = 0x0E;
    ring_buffer
        .write(term_id, &source_buffer, 0, terminate_len as IndexT)
        .unwrap();
}
