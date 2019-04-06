#[cfg(test)]
#[allow(dead_code, unused)]
mod tests {
    use coalescing_buffer::ring::{new_ring_buffer, Receiver, Sender};

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

    #[derive(Debug, Clone, PartialEq, Copy, Eq)]
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

    fn create_buf(
        capacity: usize,
    ) -> (
        Sender<usize, MarketSnapshot>,
        Receiver<usize, MarketSnapshot>,
    ) {
        new_ring_buffer(capacity)
    }

    #[test]
    fn should_reject_new_keys_when_full() {
        let (sender, receiver) = create_buf(2);
        sender.offer(1, BP_SNAPSHOT.clone());
        sender.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(!sender.offer(4, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, sender.size());
    }

    #[test]
    fn should_accept_existing_keys_when_full() {
        let (sender, receiver) = create_buf(2);
        sender.offer(1, BP_SNAPSHOT.clone());
        sender.offer(2, VOD_SNAPSHOT_1.clone());

        assert!(sender.offer(2, VOD_SNAPSHOT_2.clone()));
        assert_eq!(2, sender.size());
    }

    #[test]
    fn should_return_single_value() {
        let (sender, receiver) = create_buf(2);

        add_key_value(&sender, BP_SNAPSHOT.clone());
        assert_contains(&receiver, vec![BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn should_return_two_values_with_different_keys() {
        let (sender, receiver) = create_buf(2);
        add_key_value(&sender, BP_SNAPSHOT.clone());
        add_key_value(&sender, VOD_SNAPSHOT_1.clone());

        assert_contains(&receiver, vec![BP_SNAPSHOT.clone(), VOD_SNAPSHOT_1.clone()]);
    }

    #[test]
    fn should_update_values_with_equal_keys() {
        let (sender, receiver) = create_buf(2);
        add_key_value(&sender, VOD_SNAPSHOT_1.clone());
        add_key_value(&sender, VOD_SNAPSHOT_2.clone());
        assert_contains(&receiver, vec![VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn should_not_update_values_without_keys() {
        let (sender, receiver) = create_buf(2);
        add_value(&sender, VOD_SNAPSHOT_1.clone());
        add_value(&sender, VOD_SNAPSHOT_2.clone());
        assert_contains(
            &receiver,
            vec![VOD_SNAPSHOT_1.clone(), VOD_SNAPSHOT_2.clone()],
        );
    }

    #[test]
    fn should_update_values_with_equal_keys_and_preserve_ordering() {
        let (sender, receiver) = create_buf(2);
        add_key_value(&sender, VOD_SNAPSHOT_1.clone());
        add_key_value(&sender, BP_SNAPSHOT.clone());
        add_key_value(&sender, VOD_SNAPSHOT_2.clone());

        assert_contains(&receiver, vec![VOD_SNAPSHOT_2.clone(), BP_SNAPSHOT.clone()]);
    }

    #[test]
    fn should_not_update_values_if_read_occurs_between_values() {
        let (sender, receiver) = create_buf(2);

        add_key_value(&sender, VOD_SNAPSHOT_1.clone());
        assert_contains(&receiver, vec![VOD_SNAPSHOT_1.clone()]);

        add_key_value(&sender, VOD_SNAPSHOT_2.clone());
        assert_contains(&receiver, vec![VOD_SNAPSHOT_2.clone()]);
    }

    #[test]
    fn should_return_only_the_maximum_number_of_requested_items() {
        let (sender, receiver) = create_buf(10);
        add_value(&sender, BP_SNAPSHOT.clone());
        add_value(&sender, VOD_SNAPSHOT_1.clone());
        add_value(&sender, VOD_SNAPSHOT_2.clone());

        let snapshots = receiver.poll(2);
        assert_eq!(2, snapshots.len());
        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_1, snapshots.get(1).unwrap());

        let snapshots = receiver.poll(1);
        assert_eq!(1, snapshots.len());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(0).unwrap());

        assert_is_empty(&receiver);
    }

    #[test]
    fn should_return_all_items_without_request_limit() {
        let (sender, receiver) = create_buf(10);
        add_value(&sender, BP_SNAPSHOT.clone());
        add_key_value(&sender, VOD_SNAPSHOT_1.clone());
        add_key_value(&sender, VOD_SNAPSHOT_2.clone());

        let snapshots = receiver.poll_all();
        assert_eq!(2, snapshots.len());

        assert_eq!(&BP_SNAPSHOT, snapshots.get(0).unwrap());
        assert_eq!(&VOD_SNAPSHOT_2, snapshots.get(1).unwrap());

        assert_is_empty(&receiver);
    }

    #[test]
    fn should_count_rejections() {
        let (sender, receiver) = create_buf(2);
        assert_eq!(0, sender.rejection_count());

        sender.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(0, sender.rejection_count());

        sender.offer(1, VOD_SNAPSHOT_1.clone());
        assert_eq!(0, sender.rejection_count());

        sender.offer(1, VOD_SNAPSHOT_2.clone());
        assert_eq!(0, sender.rejection_count());

        sender.offer_value_only(BP_SNAPSHOT.clone());
        assert_eq!(1, sender.rejection_count());

        sender.offer(2, BP_SNAPSHOT.clone());
        assert_eq!(2, sender.rejection_count());
    }

    #[test]
    fn should_use_object_equality_to_compare_keys() {
        let (sender, receiver) = new_ring_buffer::<String, MarketSnapshot>(2);

        sender.offer(String::from("boo"), BP_SNAPSHOT.clone());
        sender.offer(String::from("boo"), BP_SNAPSHOT.clone());

        assert_eq!(1, sender.size());
    }

    fn assert_is_empty(receiver: &Receiver<usize, MarketSnapshot>) {
        assert_contains(receiver, vec![]);
    }

    fn add_key_value(sender: &Sender<usize, MarketSnapshot>, snapshot: MarketSnapshot) {
        assert!(sender.offer(snapshot.instrument_id, snapshot));
    }

    fn assert_contains(
        receiver: &Receiver<usize, MarketSnapshot>,
        expected: Vec<MarketSnapshot>,
    ) -> bool {
        let actual = receiver.poll_all();
        println!("Contains 1");
        let ret = (actual.len() == expected.len()) && // zip stops at the shortest
            actual.iter().zip(expected).all(|(a, b)| a == &b);
        println!("Contains 2");
        return ret;
    }

    fn add_value(sender: &Sender<usize, MarketSnapshot>, snapshot: MarketSnapshot) {
        assert!(sender.offer_value_only(snapshot));
    }
}
