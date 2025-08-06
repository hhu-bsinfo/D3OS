# NE2000 Network Card Driver for D3OS

## TODO:

- [ ] READ https://en.wikipedia.org/wiki/Ethernet_frame
- [ ] check the ne2000.cpp impl
- [x] rewrite the call for receive and overflow with AtomicBool values
- [ ] check overwrite method
- [ ] rewrite of nettest application in rust
- [ ] check boundaries for receive buffer
- [ ] check page size page buffer
- [ ] check if the ovwe bit gets actually set at the initialization of the nic
- [ ] check if packets bigger than max. Ethernet size don't get processed in the receive method and if so do the same for smoltcp so that no buffer gets enqueued

## Queues:

### send_queue

- MPSC queue
- wrapped into a Mutex
- producer = Sender,

- consumer = receiver
  - Mutex ensures exclusive access when an interrupt handler and a polling path might drain the queue
- every TxToken can enqueue a DMA buffer without blocking,
  while the driver’s service loop dequeues those buffers and
  recycles the memory when the card finishes transmission.

### receive_buffer_empty

### BNDY and CURR Register

- BNDY : read pointer, first page not yet processed, driver owned
- all pages up to but not including the packet which is being filled get freed
- CURR : first page page being written , hardware owned
- BNDY also used when packet is removed
- CURR = "where the NIC will write next" ← hardware write pointer
- BNRY = "last page I've already handled" ← driver read pointer

- When both point to the same page the ring is empty;
  when CURR catches up to BNRY the ring is full and reception stops to avoid overwriting unread data
