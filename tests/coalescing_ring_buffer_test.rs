#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MarketSnapshot {
        instrument_id: usize,
        bid: isize,
        ask: isize,
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
        CoalescingRingBuffer::new(capacity);
    }


    #[test]
    fn should_correctly_increase_the_capacity_to_the_next_higher_power_of_two() {
        check_capacity(1024, create_buffer(1023));
    }

    fn check_capacity(capacity: usize, buffer: CoalescingBuffer<Long, MarketSnapshot>) -> bool {
        assert_eq!(capacity, buffer.capacity());

        for i in 0..capacity {
            success = buffer.offer(MarketSnapshot::new(i, i, i));
            assert!(success);
        }
    }

}
