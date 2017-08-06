extern crate coalescing_ring_buf;


#[cfg(test)]
mod tests {
    use coalescing_ring_buf::*;

    #[derive(Debug)]
    struct MarketSnapshot {
        pub instrument_id: usize,
        pub bid: isize,
        pub ask: isize,
    }

    impl MarketSnapshot {
        pub fn new(instrument_id: usize, best_bid: isize, best_ask: isize) -> Self {
            MarketSnapshot {
                instrument_id: instrument_id,
                bid: best_bid,
                ask: best_ask,
            }
        }
    }

    fn create_buffer(capacity: usize) -> CoalescingRingBuffer<usize, MarketSnapshot> {
        CoalescingRingBuffer::new(capacity)
    }


    #[test]
    fn should_correctly_increase_the_capacity_to_the_next_higher_power_of_two() {
        check_capacity(1024, &mut create_buffer(1023));
        check_capacity(1024, &mut create_buffer(1024));
        check_capacity(2048, &mut create_buffer(1025));
    }

    fn check_capacity(capacity: usize, buffer: &mut CoalescingRingBuffer<usize, MarketSnapshot>) {
        assert_eq!(capacity, buffer.capacity());
        for i in 0..capacity {
            let success = buffer.offer(0, MarketSnapshot::new(i, i as isize, i as isize));
            assert!(success);
        }
    }

    #[test]
    fn should_correctly_report_size() {

        let VOD_SNAPSHOT_1 = MarketSnapshot::new(1, 3, 4);
        let VOD_SNAPSHOT_2 = MarketSnapshot::new(1, 5, 6);
        let BP_SNAPSHOT = MarketSnapshot::new(2, 7, 8);
        
        let mut buffer = create_buffer(2);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer(BP_SNAPSHOT.instrument_id,BP_SNAPSHOT);
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer(VOD_SNAPSHOT_1.instrument_id, VOD_SNAPSHOT_1);
        assert_eq!(2, buffer.size());
        assert!(!buffer.is_empty());
        assert!(buffer.is_full());

        let buf = buffer.poll(1);
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        let buf = buffer.poll(1);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }
}
