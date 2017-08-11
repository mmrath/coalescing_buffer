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

        buffer.offer_value_only(BP_SNAPSHOT.clone());
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

    #[test]
    fn should_not_update_values_without_keys() {
        let mut buffer = create_buffer(2);
        add_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        add_value(&mut buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(&mut buffer, vec![VOD_SNAPSHOT_1.clone(), VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn should_update_values_with_equal_keys_and_preserve_ordering() {
        let mut buffer = create_buffer(2);
        add_key_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&mut buffer, BP_SNAPSHOT.clone());
        add_key_value(&mut buffer, VOD_SNAPSHOT_2.clone());

        assert_contains(&mut buffer, vec![VOD_SNAPSHOT_2.clone(), BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn should_not_update_values_if_read_occurs_between_values() {
        let mut buffer = create_buffer(2);

        add_key_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        assert_contains(&mut buffer, vec![VOD_SNAPSHOT_1.clone()]);

        add_key_value(&mut buffer, VOD_SNAPSHOT_2.clone());
        assert_contains(&mut buffer, vec![VOD_SNAPSHOT_2.clone()]);
    }


    #[test]
    fn should_return_only_the_maximum_number_of_requested_items() {
        let mut buffer = create_buffer(10);
        add_value(&mut buffer, BP_SNAPSHOT.clone());
        add_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        add_value(&mut buffer, VOD_SNAPSHOT_2.clone());

        let snapshots = buffer.poll(2);
        assert_eq!(2, snapshots.len());
        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_1, snapshots.get(1).unwrap());

        let snapshots = buffer.poll(1);
        assert_eq!(1, snapshots.len());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(0).unwrap());

        assert_is_empty(&mut buffer);
    }


    #[test]
    fn should_return_all_items_without_request_limit() {
        let mut buffer = create_buffer(10);
        add_value(&mut buffer, BP_SNAPSHOT.clone());
        add_key_value(&mut buffer, VOD_SNAPSHOT_1.clone());
        add_key_value(&mut buffer, VOD_SNAPSHOT_2.clone());

        let snapshots = buffer.poll_all();
        assert_eq!(2, snapshots.len());

        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(1).unwrap());

        assert_is_empty(&mut buffer);
    }


    #[test]
    fn should_count_rejections() {
        let mut buffer = create_buffer(2);
        assert_eq!(0, buffer.rejection_count());

        buffer.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer(1, VOD_SNAPSHOT_1.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer(1, VOD_SNAPSHOT_2.clone());
        assert_eq!(0, buffer.rejection_count());

        buffer.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(1, buffer.rejection_count());

        buffer.offer(2, BP_SNAPSHOT.clone());
        assert_eq!(2, buffer.rejection_count());
    }

    #[test]
    fn should_use_object_equality_to_compare_keys() {
        let mut buffer: CoalescingRingBuffer<String, MarketSnapshot> = CoalescingRingBuffer::new(2);

        buffer.offer(String::from("boo"), BP_SNAPSHOT.clone());
        buffer.offer(String::from("boo"), BP_SNAPSHOT.clone());

        assert_eq!(1, buffer.size());
    }

    fn assert_is_empty(buffer: &mut CoalescingRingBuffer<usize, MarketSnapshot>) {
        assert_contains(buffer, vec![]);
    }


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

    fn add_value(buffer: &mut CoalescingRingBuffer<usize, MarketSnapshot>, snapshot: MarketSnapshot) {
        assert!(buffer.offer_value_only(snapshot));
    }
}
