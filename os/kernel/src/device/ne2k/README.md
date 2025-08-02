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
