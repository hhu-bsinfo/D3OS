# mlx4 / ibverbs data path bugs

Bugs found while getting `rdma-bench` to run between D3OS on `ib2` and `rust-rdma-bench` on
`ib1` over a real InfiniBand fabric, on branch `benchmark-tool`.

Both directions (D3OS client → Linux server, and Linux client → D3OS server) now complete a
1000-iteration, 64-byte RC bandwidth run.

Most of these bugs masked one another: each one made the card silently do nothing, so the
symptom was identical no matter which was actually in the way. The order below is the order in
which they had to be fixed, not the order they were found.

---

## 1. An all-zero GID was advertised to the peer

**Symptom.** Linux client against D3OS server: `WC error: 12` (`IBV_WC_RETRY_EXC_ERR`) — the
transport retry counter ran out, as if D3OS never responded. D3OS's own log showed the queue
pair reaching RTS with a correct context.

**Cause.** `ibv_query_gid` is a stub returning an all-zero GID:

```rust
// os/library/ibverbs/src/ibverbs_sys.rs
pub fn ibv_query_gid(...) -> Result<ibv_gid> {
    // TODO: figure out how to actually do this as the Nautilus driver can't
    Ok(ibv_gid { raw: [0; 16] })
}
```

`PreparedQueuePair::endpoint()` wrapped that in `Some(...)` unconditionally. A peer that
receives a GID takes the global-routing branch of its handshake — `ah_attr.is_global = 1`,
`grh.dgid = 0::0` — and puts a GRH addressed to the null GID on every packet. Meanwhile the
driver hardcodes `context.set_primary_grh(false)` (`queue_pair.rs`) and never programs an
`mgid_index`, hop limit or `rgid`, so D3OS is LID-routed only and cannot be addressed by GID at
all. The peer's packets were undeliverable, never acknowledged, and it exhausted its retries.

**Fix.** `endpoint()` advertises a GID only when one actually exists:

```rust
let gid = (self.ctx.gid.raw != [0u8; 16]).then_some(self.ctx.gid);
```

Self-healing: once `ibv_query_gid` is implemented *and* the driver programs the GRH, the GID is
advertised again with no further change.

**How it was found.** Printing the local endpoint alongside the remote one. The LID was correct
(2, with `ib1` at 1); the GID was sixteen zero bytes.

---

## 2. `ibv_post_send` never sent the work requests

**Symptom.** The client posted 32 sends, all returning `Ok`, then hung forever polling for
completions. Nothing was transmitted.

**Cause.** The request was passed as raw struct bytes rather than encoded:

```rust
// os/library/ibverbs/src/ibverbs_sys.rs — before
uverbs(device_handle, OpPostSend, UserSlice::from_ref(&req), UserSlice::EMPTY)
```

`PostSendRequest::wrs` is a `Vec`, so those bytes are a pointer, a length and a capacity — not
the work requests. The kernel decodes the buffer with `bincode::decode_from_slice`
(`infiniband/uverbs.rs`). `ibv_post_recv`, three functions below, does it correctly with
`bincode::encode_to_vec` + `UserSlice::from_slice`.

The failure mode is what made this expensive: bincode-decoding the raw struct bytes does not
error, it *succeeds* and yields an empty `wrs`. So `post_send` wrote nothing to the send queue,
never rang the doorbell, and returned success.

**Fix.** Encode the request the same way `ibv_post_recv` does.

**How it was found.** A trace at the top of `QueuePair::post_send` printing `wrs.len()`:
`QP 64: post_send with 0 work request(s)`, thirty-two times.

---

## 3. MTT entries were written through the virtual-function command path

**Symptom.** With sends now reaching the driver and correct WQEs written at the correct
offsets, the card still did nothing: no transmission, no completions, and — on the receive side
— RNR NAKs to the peer, meaning the card could not find a posted receive buffer.

**Cause.** `alloc_mtt_for_pages` populated the memory translation table with the `WRITE_MTT`
command. The reference driver only does that on a *virtual function*, which has no access to
ICM. On a physical function — which is what D3OS drives — `mlx4_write_mtt_chunk` resolves the
entry with `mlx4_table_find` and writes it **straight into ICM host memory**:

```c
mtts = mlx4_table_find(&priv->mr_table.mtt_table, mtt->offset + start_index, &dma_handle);
for (i = 0; i < npages; ++i)
    mtts[i] = cpu_to_be64(page_list[i] | MLX4_MTT_FLAG_PRESENT);
```

ICM is ordinary host memory the card reads by DMA, so the entries simply never appeared. Every
buffer the card reaches through the MTT was therefore unreachable: send and receive WQE
buffers, the completion queue ring, and the event queue ring. Command mailboxes kept working
throughout because they are passed to the card as raw physical addresses in the HCR and bypass
ICM entirely — which is why every context read back perfectly while nothing functioned.

**Fix.** Write the entries directly into the ICM memory backing the table, via a new
`IcmTable::host_bytes_mut` helper, with a compiler fence before anything points the card at
them.

**How it was found.** Reading the entries back out of the host memory backing ICM — no card
involvement — and seeing an untouched fill pattern (`70 a4 30 91` repeating) where the physical
addresses should have been. Confirmation after the fix was unambiguous: the event queue ring
came back filled with `0xee`, the firmware's own initialisation pattern, and the consumer index
stopped at 3 real events instead of walking all 4096 entries.

### Related, fixed alongside

- **`WRITE_MTT` mailbox units.** While that path was still in use, its offset field was being
  given a byte offset. The command addresses the table in *entries*
  (`inbox[0] = mtt->offset + start_index`), while only the MPT's and queue pair context's
  `mtt_base_addr` want a byte address (`mlx4_mtt_addr` = `offset * mtt_entry_sz`). The command
  path and its `WriteMttCommand` mailbox struct have since been deleted, so this is history
  rather than a live concern.
- **`get_physical_address` swallowed failures.** It returned `PhysAddr::zero()` on a failed
  translation, which would have handed the card a null DMA target with no error anywhere. It
  now returns `Result`. `alloc_mtt_for_pages` resolves its pages against the kernel address
  space directly and reports an unmapped page rather than silently emitting a null entry.

**Not done: MTT segment alignment.** `__mlx4_alloc_mtt_range` returns
`seg * (1 << log_mtts_per_seg)` and reserves `1 << ceil(log2(npages))` entries, so in the
reference driver every range is segment-aligned and no two ranges share a segment.
`alloc_mtt_for_pages` still packs ranges densely (`self.offset += pages.len()`). This has not
caused an observed problem — the hardware only requires the byte address to be 8-byte aligned,
which dense packing satisfies — but it is a deliberate divergence from the reference driver and
worth revisiting if MTT-related corruption ever reappears.

---

## 4. The completion buffer handed to the driver was always empty

**Symptom.** The card was demonstrably producing completions — a valid CQE sat in the ring, for
the right QP, with `is_send` set and opcode `0x0a` — and `ibv_poll_cq` still reported zero.

**Cause.** In the `PollCq` handler:

```rust
// os/kernel/src/infiniband/uverbs.rs — before
let mut wc_buf = Vec::with_capacity(supported_len);
let wc_count = uverbs_poll_cq(device_handle, req.cq_num, &mut wc_buf)?;
```

`Vec::with_capacity` reserves space but leaves the length at **zero**. `uverbs_poll_cq` takes a
`&mut [ibv_wc]`, so the slice was empty and `CompletionQueue::poll`'s loop

```rust
while completions < wc.len() { ... }
```

never ran a single iteration. It returned 0 unconditionally regardless of what the card had
produced.

**Fix.** `vec![ibv_wc::default(); supported_len]`.

**How it was found.** Dumping the raw bytes of the CQE at the consumer index from inside the
"no completion" path — it showed a perfectly valid completion that the poll loop was never
looking at.

---

## 5. The receive queue's tail never advanced

**Symptom.** The server accepted traffic and produced completions, reposted receives, and then
failed with `receive queue would overflow` after exactly 256 posts.

**Cause.** `poll_one` advances the tail by the chain size recorded for that WQE:

```rust
let chain_size = qp.query_chain_size(cqe.wqe_index() as usize, cqe.is_send());
qp.advance_receive_queue_by(chain_size);
```

`post_receive` had lost its `update_chain_size` call in the refactor, so `rq.meta[i].1` stayed
zero and the tail never moved. `would_overflow` compares `head - tail` against `max_post`, so
it fired after exactly `max_post` posts no matter how many had actually completed.

**Fix.** Restore `self.rq.update_chain_size(index, 1)` — every receive WQE produces its own
completion, so the chain is always one entry long.

---

## 6. Applications silently linked stale libraries

**Symptom.** A kernel panic, `assertion failed: user_in.size >= size`, after a change to a type
shared between kernel and userspace.

**Cause.** Each application's `compile` task is gated on a `files_modified` condition listing
the libraries it depends on. Every RDMA application listed only the four from the template
(`runtime`, `terminal`, `concurrent`, `syscall`) and **not** `ibverbs`, `rdma`, `network`,
`time`, `cpu`, `naming` or `mm`. An unlisted dependency does not rebuild late — it *never*
rebuilds. The kernel was rebuilt from new sources while the application kept linking an old
copy of the library, so the two sides disagreed about a structure that crosses the syscall
boundary as raw bytes.

**Fix.** Regenerated each condition from the crate's real path dependencies, in
`rdma-bench`, `rdma/mlx4`, `perftest`, `infiniband-diags/ibping` and `infiniband-diags/ibstat`,
with a comment explaining why the list has to stay complete.

**Worth knowing.** This had been silently producing mixed builds for some time; any library-only
edit before this fix may not have been in the image that was tested. The same pattern likely
affects the non-RDMA applications in the tree.

## 7. Memory regions overwrote the firmware's own, taking the port down after ~8 runs

**Symptom.** After a few runs the port is in state `Initializing`, seen both from D3OS and from
`ib1`. Every RDMA application then fails at `ibv_open_device`, because `Context::with_device`
refuses a port that is not `ACTIVE` or `ARMED`. It happened after 8 runs at a 64 KB message size
and after 7 at 64 bytes.

**Cause.** `alloc_dmpt` allocated an entry in the data memory protection table by handing the
allocator's number to `DmptEntry::set_key`:

```rust
// os/kernel/src/device/mlx4/icm.rs — before
dmpt.set_key(offsets.alloc_dmpt().try_into().unwrap());   // 256, 512, 768, ...
```

`set_key` applies `key_to_hw_index`, which rotates by 8 bits — so 256 became index **1**, 512
became **2**, and so on, one further entry per registered memory region. The card reports
`reserved MPTs: 256`: indices 0 to 255 belong to the firmware. Every memory region D3OS ever
registered was therefore programmed on top of one of the firmware's own, with `SW2HW_MPT` given
the index and returning `Ok` each time.

The reference driver goes the other way round. `mlx4_mr_alloc` allocates an *index* from a bitmap
initialised with `dev->caps.reserved_mrws` as its lower bound and derives the key from it with
`hw_index_to_key`; only the index is ever passed to `SW2HW_MPT`, and only the key is ever handed
to an application.

Runs 1 to 7 overwrote entries the firmware was not using at that moment. Run 8 reached one it
was, and the port went down on the next `post_send`.

**Fix.** Allocate the index directly — `Offsets::alloc_dmpt` counts up by one from
`1 << log2_rsvd_mrws` instead of by 256, and `alloc_dmpt` calls `set_index`. `set_key` is gone;
`key()` remains, since the key is derived from the index and not the reverse.

**How it was found.** Two things, in order. Decoding the async event queue produced `port 1 is
now down`, and draining it after every verb pinned it to `OpPostSend`. Then the memory key in the
log gave the index away: run 7 logged `mem key 1792`, run 8 `mem key 2048`, and those are
`hw_index_to_key(7)` and `hw_index_to_key(8)` — the driver was working at the very bottom of a
table whose first 256 entries were not its own.

### What the symptom looked like before that was understood

**The port state is the aftermath, not the fault.** The card posts a port-down event in the
middle of a run:

```
[70.020] Create dMTP for addr: 0x00003f0000000078, size: 0x10000
[70.020] Create MTT mappings for PageRange { 0x3f0000000000 .. 0x3f0000011000 }   (17 pages)
[70.022] memory region of size 69632 with mem key 2816 created successfully
[70.050] WRN  port 1 is now down
[70.529] ERR  work completion error: (QPN 74, WQE 0, syndrome TransportRetryExceededError)
[70.530] ERR  ... WQE 1..31, syndrome WrFlushError
```

The port was healthy 180 ms earlier — `device::open()` only succeeds on an `ACTIVE`/`ARMED`
port — and the queue pair had reached RTS with every command returning `Ok`. The transport
retry error on WQE 0 is the consequence: the send went out onto a port that was already going
down, and the other 31 work requests flushed behind it.

**The subnet manager's view matches exactly.** From `opensm.0xe41d2d030017fda1.log` on `ib1` for
the same boot:

```
13:17:21  SM port is down / Entering DISCOVERING state    <- D3OS boots, resets the card
13:17:41  SM port is up / MASTER / SUBNET UP
13:17:51 .. 13:18:31  SUBNET UP                          <- five clean sweeps, D3OS answers
13:18:42  ERR 3113: MAD completed in error (IB_TIMEOUT): SubnGet(NodeInfo)
          drop_mgr_remove_port: Removed port GUID:0xf452140300784231 LID range [2,2]
13:18:52 .. 13:27:52  the same timeout every 10 s, forever
```

The first `NodeInfo` timeout coincides with the port-down event. Since D3OS creates no QP0 and
has no MAD agent, `NodeInfo` is answered by the HCA firmware's own SMA — so once the port is
down, nothing answers, OpenSM drops the node, and because nothing in D3OS ever re-initialises a
port it stays in `Initializing` for the rest of the session.

**What it is not.**

- Not a wedged card: `QueryPort`, `MadIfc`, `Hw2SwMpt`, `Any2RstQp` and `Hw2SwCq` all still
  return `Ok` afterwards. The firmware is alive; only the link is gone.
- Not a rejected DMA: `ib2` runs the card through VFIO, and its `dmesg` has no DMAR/IOMMU fault,
  no PCIe AER error and no vfio error for any of the failing runs. (This only rules out a target
  *outside* guest RAM — VFIO maps all of it.)
- Not a subnet-manager problem: OpenSM is running throughout and sweeps every 10 s.
- Not recoverable by the driver: the port stays down until QEMU exits and the host's `mlx4_core`
  re-initialises the card, which always succeeds — consistent with the damage being to state the
  firmware holds in ICM.

The instrumentation that made all of this visible is worth keeping: the async event queue is now
decoded rather than discarded, it is drained after every verb so an event is logged next to the
operation that provoked it, the internal error buffer (`err_bar 0 + 0x1f020`, 16 words) is polled
the way `mlx4_catas` does, and `alloc_mtt_for_pages` reads its entries back out of ICM and
rejects a translation that is zero or unaligned.

## 8. Error on Linux Client
**Symptom.** When running `rdma-bench server` the linux client still sometimes reports an error
`error: WC error: 13 vendor_err=135`. The error occurred after 3 successful runs. It may be related to Bug 7

**Ruled out.** The queue pair context reads back `rnr_retry 6, min_rnr_nak 16, ack timeout 16`
after the transition to RTS, so those values do reach the card. Status 13 also means the packets
arrived at a live queue pair whose receive queue was empty, which is the opposite of bug 7's dead
port — so unless the two coincide, this is a separate fault. The open candidates are the receive
window running dry (`bench/bandwidth.rs` posts `tx_depth` receives and reposts one per
completion) and the receive queue's tail drifting away from the card's.

---

## Smaller defects fixed along the way

- **`post_receive` corrupted the following WQE.** The refactor replaced
  `get_data_segment(memory, index, sge_index)` (which addresses within one WQE) with
  `get_element(memory, index + sge_index)` (which addresses whole WQEs) and dropped the
  `if sge_index < max_gs` guard. With `max_gs == 1` an RQ WQE *is* one data segment, so every
  post wrote an invalid-lkey terminator over the next entry. `mlx4_ib_post_recv` only writes the
  terminator when `i < rq.max_gs`.
- **Inverted WQE stamping in `post_send`.** `if !curr.next.is_null()` (there *are* more work
  requests) became `if peekable.peek().is_none()` (there are *none*), contradicting the comment
  directly above it and `mlx4_ib_post_send`.
- **Byte order double-swap in the send queue pre-initialisation.** `vlan_cv_f_ds` is a
  `U32<BigEndian>`, so `.into()` already swaps; `u32::to_be(...).into()` swapped twice and put
  the descriptor size into `qpn_vlan.vlan_tag` instead of `fence_size`.
- **Event queue armed with no interrupt handler.** `init_eqs` armed the queue while
  `EventQueue::new` is called with no IRQ, so once events started arriving the unacknowledged
  interrupts livelocked the system. It now rings disarmed and is drained by polling from
  `CompletionQueue::poll`, which had been commented out entirely.
- **Structures crossing the syscall boundary were `repr(Rust)`.** `ibv_qp_attr`, `ibv_ah_attr`,
  `ibv_gid`, `ibv_global_route`, `ibv_qp_cap`, `ibv_port_attr` and `ibv_sge` are now `repr(C)`.
  Their layout was never observed to differ between the two builds, but nothing guarantees it.
- **Barrier bytes could sit unsent.** `TcpStream::write` only hands bytes to the network stack
  and `os/library/network` exposes no flush. After a barrier the benchmark goes straight into a
  tight polling loop and stops touching the socket, leaving the peer waiting on a byte still in
  a send buffer. `Conn::write_all` now yields once after writing.

---

## Known remaining issues

Not fixed, in rough order of how soon they will matter.

- **Memory regions use the reserved lkey and physical addressing.** `alloc_dmpt` hands out
  `caps.reserved_lkey()` and `copy_from_sge` translates the SGE address to a physical one, so
  local data access bypasses the MPT/MTT and the card reads a physically *contiguous* range.
  Any buffer whose pages are not physically contiguous will be transferred incorrectly. It does
  not show up at 64 bytes; it will as soon as a message crosses a page boundary — a 4096-byte
  buffer that is not page-aligned already spans two pages.
- **`retry_count` and `ack_req_freq` are never programmed** into the queue pair context
  (`// TODO: ack_req_freq, next_send_psn, retry_count`). The context reads back with
  `params1` bits 31:28 and 18:16 zero, where Linux uses `MLX4_IB_ACK_REQ_FREQ` = 8 and
  `attr->retry_cnt`. D3OS as a requester therefore gets zero transport retries.
- **Teardown converts recoverable errors into panics.** `QueuePair::destroy` takes `self` by
  value and only clears `memory` at the very end, so any error on the way out reaches
  `Drop`, which does `panic!("please destroy instead of dropping")`. `CompletionQueue` and
  `EventQueue` share the pattern.
- **Completion queue size is not rounded to a power of two.** `create` does
  `set_log_size(num_entries.ilog2())` while `get_next_cqe_sw` masks with `num_entries - 1`. A
  `--tx-depth` that is not a power of two (3 → 6 entries) makes `ilog2` truncate to 4 while the
  driver masks with 5, and the card and driver disagree about the buffer. Linux does
  `entries = roundup_pow_of_two(entries + 1)`.
- **`pri_path.ackto` does not track `attr.timeout`.** The context reads back `0x80` (timeout 16)
  while the builder requests 4, and the value did not change when the input did. Harmless for a
  responder; unexplained.
- **The event queue is drained on the completion-polling path.** `CompletionQueue::poll` calls
  `handle_events` before looking for completions, because nothing else ever consumes the ring
  and it would otherwise fill. That is one extra memory read per `ibv_poll_cq`, on the hot path,
  and it belongs on a separate thread or behind interrupts before latency is measured.
- **The special queue pair base is derived from the wrong capability.** `Offsets::init` computes
  `base_qpn` — where QP0 and QP1 live — from `log2_rsvd_cqs`, while `next_qpn` starts at
  `1 << log2_rsvd_qps` and only ever increases, since queue pair numbers are never reused. If the
  two ranges overlap, the ordinary allocator eventually hands out QP0, which would look exactly
  like the card refusing to answer the subnet manager after N runs.
- **BlueFlame is disabled.** `USE_BLUEFLAME` in `device/mlx4/mod.rs` is `false`. Because
  `post_send` took that path for *every* single work request post, the ordinary doorbell path
  had never been exercised. It is worth re-enabling and testing separately now that the doorbell
  path is known good.
