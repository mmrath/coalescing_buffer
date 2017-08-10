extern crate rbuf;


#[cfg(test)]
mod tests {
    use rbuf::*;

    static VOD_SNAPSHOT_1: MarketSnapshot = MarketSnapshot {
        instrument_id: 1,
        bid: 3,
        ask: 4,
    };
    static VOD_SNAPSHOT_2: MarketSnapshot = MarketSnapshot {
        instrument_id: 1,
        bid: 5,
        ask: 6,
    };
    static BP_SNAPSHOT: MarketSnapshot = MarketSnapshot {
        instrument_id: 2,
        bid: 7,
        ask: 8,
    };

    #[derive(Debug, Clone, PartialEq)]
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
        let mut buffer = create_buffer(2);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer(BP_SNAPSHOT.instrument_id, BP_SNAPSHOT.clone());
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        buffer.offer(VOD_SNAPSHOT_1.instrument_id, VOD_SNAPSHOT_1.clone());
        assert_eq!(2, buffer.size());
        assert!(!buffer.is_empty());
        assert!(buffer.is_full());

        let _ = buffer.poll(1);
        assert_eq!(1, buffer.size());
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());

        let _ = buffer.poll(1);
        assert_eq!(0, buffer.size());
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }


    #[test]
    fn should_reject_new_keys_when_full() {
        let mut buffer = create_buffer(2);
        buffer.offer(1, BP_SNAPSHOT.clone());
        buffer.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(!buffer.offer(4, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, buffer.size());
    }

    #[test]
    fn should_accept_existing_keys_when_full() {
        let mut buffer = create_buffer(2);
        buffer.offer(1, BP_SNAPSHOT.clone());
        buffer.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(buffer.offer(2, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, buffer.size());
    }


    #[test]
    fn should_return_single_value() {
        let mut buffer = create_buffer(2);

        add_key_value(&mut buffer, BP_SNAPSHOT.clone());
        assert_contains(&mut buffer, vec![BP_SNAPSHOT.clone()]);
    }


    #[test]
    fn should_return_two_values_with_different_keys() {
        let mut buffer = create_buffer(2);
        add_key_value(&mut buffer, BP_SNAPSHOT.clone());
        add_key_value(&mut buffer, VOD_SNAPSHOT_1.clone());

        assert_contains(&mut buffer, vec![BP_SNAPSHOT.clone(), VOD_SNAPSHOT_1.clone()]);
    }


    #[test]
    fn should_update_values_with_equal_keys() {
        let mut buffer = create_buffer(2);
        add_key_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&mut buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(&mut buffer, vec![VOD_SNAPSHOT_2.clone()]);
    }

    /*
    #[test]
    fn shouldUpdateValuesWithEqualKeysAndPreserveOrdering() {
        let mut buffer = create_buffer(2);
        addKeyAndValue(&mut buffer, VOD_SNAPSHOT_1.clone());
        addKeyAndValue(&mut buffer, BP_SNAPSHOT.clone());
        addKeyAndValue(&mut buffer, VOD_SNAPSHOT_2.clone());

        assertContains(&mut buffer, vec![VOD_SNAPSHOT_2.clone(), BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn shouldNotUpdateValuesIfReadOccursBetweenValues() {
        let mut buffer = create_buffer(2);

        addKeyAndValue(&mut buffer, VOD_SNAPSHOT_1.clone());
        assertContains(&mut buffer, vec![VOD_SNAPSHOT_1.clone()]);

        addKeyAndValue(&mut buffer, VOD_SNAPSHOT_2.clone());
        assertContains(&mut buffer, vec![VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn shouldReturnOnlyTheMaximumNumberOfRequestedItems() {
        addValue(BP_SNAPSHOT);
        addValue(VOD_SNAPSHOT_1);
        addValue(VOD_SNAPSHOT_2);

        List < MarketSnapshot > snapshots = new ArrayList < MarketSnapshot > ();
        assertEquals(2, buffer.poll(snapshots, 2));
        assertEquals(2, snapshots.size());
        assertSame(BP_SNAPSHOT, snapshots.get(0));
        assertSame(VOD_SNAPSHOT_1, snapshots.get(1));

        snapshots.clear();
        assertEquals(1, buffer.poll(snapshots, 1));
        assertEquals(1, snapshots.size());
        assertSame(VOD_SNAPSHOT_2, snapshots.get(0));

        assertIsEmpty();
    }

    #[test]
    fn shouldReturnAllItemsWithoutRequestLimit() {
        addValue(BP_SNAPSHOT);
        addKeyAndValue(VOD_SNAPSHOT_1);
        addKeyAndValue(VOD_SNAPSHOT_2);

        List < MarketSnapshot > snapshots = new ArrayList < MarketSnapshot > ();
        assertEquals(2, buffer.poll(snapshots));
        assertEquals(2, snapshots.size());

        assertSame(BP_SNAPSHOT, snapshots.get(0));
        assertSame(VOD_SNAPSHOT_2, snapshots.get(1));

        assertIsEmpty();
    }

    #[test]
    fn shouldCountRejections() {
        CoalescingRingBuffer < Integer, Object > buffer = new CoalescingRingBuffer < Integer, Object > (2);
        assertEquals(0, buffer.rejectionCount());

        buffer.offer(new Object());
        assertEquals(0, buffer.rejectionCount());

        buffer.offer(1, new Object());
        assertEquals(0, buffer.rejectionCount());

        buffer.offer(1, new Object());
        assertEquals(0, buffer.rejectionCount());

        buffer.offer(new Object());
        assertEquals(1, buffer.rejectionCount());

        buffer.offer(2, new Object());
        assertEquals(2, buffer.rejectionCount());
    }

    #[test]
    fn shouldUseObjectEqualityToCompareKeys() {
        CoalescingRingBuffer < String, Object > buffer = new CoalescingRingBuffer < String, Object > (2);

        buffer.offer(new String("boo"), new Object());
        buffer.offer(new String("boo"), new Object());

        assertEquals(1, buffer.size());
    }

    fn assertIsEmpty() {
        assertContains();
    }
    */

    fn add_key_value(buffer: &mut CoalescingRingBuffer<usize, MarketSnapshot>, snapshot: MarketSnapshot) {
        assert!(buffer.offer(snapshot.instrument_id, snapshot));
    }


    fn assert_contains(buffer: &mut CoalescingRingBuffer<usize,
        MarketSnapshot>, expected: Vec<MarketSnapshot>) -> bool {
        let actual = buffer.poll_all();
        println!("Contains 1");
        let ret = (actual.len() == expected.len()) && // zip stops at the shortest
            actual.iter().zip(expected).all(|(a, b)| a == &b);
        println!("Contains 2");
        return ret;
    }
}
