+-+-+-+-+-+-+
|N|e|2|0|0|0|
+-+-+-+-+-+-+

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
- [x] check if packets bigger than max. Ethernet size don't get processed in the receive method and if so do the same for smoltcp so that no buffer gets enqueued
- [x] check network/mod.rs open_socket() for transmit and rx size for packetbuffer

## Reason for slirp errors:

- because SLIRP is slow and smoltcp is single‑threaded. A huge UDP RX queue makes each poll() spend lots of time shoveling receive packets (holding the sockets lock), so egress doesn’t get serviced fast enough.
  When rx_size was shranked from 1000 → 2, the work poll() does on ingress per tick was limited, freeing time for TX to drain, so your burst to the host stopped tripping SLIRP’s “failed to send packet” path.

- QEMU “user” networking (SLIRP) is a userspace NAT with poor throughput and small queues. It’s convenient but explicitly documented as “a lot of overhead so the performance is poor.” Bursts from the guest are easy to drop/log as errors on the host side.
  wiki.qemu.org

- smoltcp drives both RX and TX in the same Interface::poll() loop. Big RX buffers mean poll() can enqueue many datagrams into UDP socket before it ever gets back to egress. That increases lock hold time on the global SocketSet and pushes out TX work. (See iface docs: poll() is the driver for interface logic.)

- Oversized buffers can reduce throughput. There’s even an open smoltcp issue showing a “sweet spot” where increasing buffer sizes past a point hurts performance due to extra work and cache pressure.

- Symptom on the host: SLIRP will complain (e.g., “Failed to send packet, ret: -1”) when it can’t keep up; people see these messages even with otherwise functional traffic. Reducing RX queue shortened each poll cycle, letting TX keep pace and avoiding SLIRP’s error path.

- So the improvement after dropping rx_size to 2 is backpressure by design:

- guest now drops excess inbound datagrams earlier (socket RX buffer fills quickly),
- which makes each poll() iteration shorter,
- which gives more CPU to egress,
- which reduces the burst pressure on SLIRP and avoids its send‑fail log spam.
- Keep RX modest, find a middle ground that matches poll rate. The smoltcp bug thread suggests there’s an “ideal” size—measure and tune.
- Increase TX payload slab (total bytes) rather than cranking metadata counts, and poll more frequently (or use poll_delay() for tight pacing). That helps TX drain smoothly without starving the system.
- For serious throughput tests, switch QEMU from SLIRP to tap/bridge networking; it bypasses SLIRP’s userspace NAT bottleneck. QEMU’s docs call out the backend options.
- TL;DR: smaller RX limited per‑tick ingress work, unblocked TX, and side‑stepped SLIRP’s bottleneck—so your bursts look “faster” and cleaner.

https://github.com/smoltcp-rs/smoltcp/issues/949?utm_source=chatgpt.com

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

Observation

- rtl8139 has no problems with sending a lot of packets (2000)
- if i use the ne2k, i get the buffer full

## changes

edited const.rs ->
