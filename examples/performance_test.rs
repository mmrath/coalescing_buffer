use coalescing_buffer::ring::{Sender, Receiver, new_ring_buffer};
use std::time::{Instant, Duration, SystemTime};
use chrono::Local;
use std::thread;
use std::sync::{Barrier, Arc};
use std::sync::atomic::{AtomicU64, Ordering};
use lazy_static::lazy_static;

static BILLION: i64 = 1000000000;
static POISON_PILL: MarketSnapshot = MarketSnapshot { instrumentId: -1, bestBid: -1, bestAsk: -1 };
static NUMBER_OF_INSTRUMENTS: i32 = 10;
static SECONDS: i32 = 1000;
lazy_static! {
    static ref STOP_WATCH: StopWatch = StopWatch {
            startingGate: Arc::new(Barrier::new(2)),
            init_time: SystemTime::now(),
            start_time: AtomicU64::new(0),
            end_time: AtomicU64::new(0),
        };
}

pub fn main() {PerformanceTest::main();}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MarketSnapshot { instrumentId: i64, bestBid: i64, bestAsk: i64 }


pub struct Producer<'a> {
    sender: Sender<i64, MarketSnapshot>,
    number_of_updates: i64,
    poisonPill: &'a MarketSnapshot,
    numberOfInstruments: i32,
    snapshots: Vec<MarketSnapshot>,
    nextSnapshot: i32,
}

impl<'a> Producer<'a> {
    pub fn createSnapshots( numberOfInstruments: usize) -> Vec<MarketSnapshot> {
        let mut snapshots: Vec<MarketSnapshot> = Vec::with_capacity(numberOfInstruments);

        for i in 0..numberOfInstruments {
            let bid = numberOfInstruments * i;
            let ask = numberOfInstruments * numberOfInstruments * i;

            snapshots.push(MarketSnapshot {
                instrumentId: i as i64,
                bestBid: bid as i64,
                bestAsk: ask as i64,
            });
        }

        return snapshots;
    }


    fn run(&mut self) {
        STOP_WATCH.producerIsReady();
        for i in 1..=self.number_of_updates {
            let next = self.nextSnapshot();
            self.put(self.nextId(i) as i64, next);
        }
        self.put(self.poisonPill.instrumentId, self.poisonPill.clone());
    }

    /**
     * simulates some instruments update much more frequently than others
     */
    fn nextId(&self, counter: i64) -> i32 {
        let mut register = counter as i32;

        for i in 1..self.numberOfInstruments {
            if register & 1 == 1 {
                return i;
            }
            register >>= 1;
        }
        return self.numberOfInstruments;
    }

    fn nextSnapshot(&mut self) -> MarketSnapshot {
        if self.nextSnapshot == self.numberOfInstruments {
            self.nextSnapshot = 0;
        }
        let current = self.nextSnapshot;
        self.nextSnapshot += 1;
        self.snapshots[current as usize].clone()
    }

    fn put(&mut self, id: i64, snapshot: MarketSnapshot) {
        let instId = snapshot.instrumentId;
        let success = self.sender.offer(id, snapshot);
        if !success {
            panic!(format!("failed to add instrument id {}", instId));
        }
    }
}


#[derive()]
pub struct StopWatch {
    startingGate: Arc<Barrier>,
    init_time: std::time::SystemTime,
    start_time: AtomicU64,
    end_time: AtomicU64,
}

impl StopWatch {
    fn consumerIsReady(&self) {
        self.awaitStart();
    }

    fn awaitStart(&self) {
        self.startingGate.wait();
    }

    fn producerIsReady(&self) {
        self.awaitStart();
        self.start_time.store(self.init_time.elapsed().unwrap().as_nanos() as u64, Ordering::Relaxed);
    }

    fn consumerIsDone(&self) {
        self.end_time.store(self.init_time.elapsed().unwrap().as_nanos() as u64, Ordering::Relaxed);
    }

    fn nanosTaken(&self) -> Duration {
        let start = self.start_time.load(Ordering::Relaxed);
        let end = self.end_time.load(Ordering::Relaxed);
        return Duration::from_nanos(end - start);
    }
}


struct Consumer<'a> {
    receiver: Receiver<i64, MarketSnapshot>,
    numberOfInstruments: i32,
    poisonPill: &'a MarketSnapshot,
    latestSnapshots: Vec<MarketSnapshot>,
    readCounter: i64,
}

impl<'a> Consumer<'a> {
    pub fn run(mut self) -> Self{
        STOP_WATCH.consumerIsReady();
        loop {
            let bucket = self.receiver.poll(self.numberOfInstruments as usize);
            for i in 0..bucket.len() {
                self.readCounter += 1;
                let snapshot = &bucket[i];
                if snapshot == self.poisonPill {
                    STOP_WATCH.consumerIsDone();
                    return self;
                }
                self.latestSnapshots[snapshot.instrumentId as usize] = snapshot.clone();
            }
            self.simulateProcessing();
            // allocation happens in poll, to be removed
        }
    }

    fn simulateProcessing(&self) {
        let now = Instant::now();
        let sleepUntil = Duration::from_nanos(10 * 1000);
        while now.elapsed() < sleepUntil {}
    }
}

struct PerformanceTest {
    receiver: Receiver<i64, MarketSnapshot>,
    sender: Sender<i64, MarketSnapshot>,
    numberOfUpdates: i64,
}

impl PerformanceTest {
    pub fn run(self) -> i64 {
        println!("testing with {} updates...", self.numberOfUpdates);

        thread::sleep(Duration::from_millis(10));

        let numberOfUpdates = self.numberOfUpdates;
        let mut producer = Producer {
            sender: self.sender,
            number_of_updates: self.numberOfUpdates,
            poisonPill: &POISON_PILL,
            numberOfInstruments: NUMBER_OF_INSTRUMENTS,
            snapshots: Producer::createSnapshots(NUMBER_OF_INSTRUMENTS as usize),
            nextSnapshot: 0,
        };
        let mut consumer = Consumer {
            receiver: self.receiver,
            numberOfInstruments: NUMBER_OF_INSTRUMENTS,
            poisonPill: &POISON_PILL,
            latestSnapshots: (0..NUMBER_OF_INSTRUMENTS).map(|_| POISON_PILL.clone()).collect(),
            readCounter: 0,
        };


        let producer_t = thread::spawn(move || { producer.run(); });
        let consumer_t = thread::spawn(move || { consumer.run() });

        let consumer = consumer_t.join().unwrap();

        return Self::computeAndPrintResults(numberOfUpdates, &consumer, STOP_WATCH.nanosTaken().as_nanos() as i64);
    }


    fn computeAndPrintResults(numberOfUpdates:i64, consumer: &Consumer, nanosTaken: i64) -> i64 {
        for i in 0..consumer.latestSnapshots.len() {
            println!("{:?}", consumer.latestSnapshots[i]);
        }

        println!("time {}s", Duration::from_nanos(nanosTaken as u64).as_secs());

        let compressionRatio = (1.0 * numberOfUpdates as f64) / consumer.readCounter as f64;
        println!("compression ratio = {:.1}", compressionRatio);

        let megaOpsPerSecond = (1000.0 * numberOfUpdates as f64) / nanosTaken as f64;
        println!("mops = {:.1}", megaOpsPerSecond);

        return megaOpsPerSecond as i64;
    }

    fn main() {
        let mut results = vec![-1, 3];
        let mut runNumber = 1;

        {
            let result = Self::run_int(runNumber, 2 * BILLION);
            runNumber += 1;
            Self::update(&mut results, result);
            thread::sleep(Duration::from_secs(5));
        }
        while !Self::areAllResultsTheSame(&results) {
            let result = Self::run_int(runNumber, 2 * BILLION);
            runNumber += 1;
            Self::update(&mut results, result);
            thread::sleep(Duration::from_secs(5));
        }
    }

    fn run_int(runNumber: i32, numberOfUpdates: i64) -> i64 {
        let (sender, receiver) = new_ring_buffer(1 << 20);

        let test = PerformanceTest {
            receiver,
            sender,
            numberOfUpdates,
        };

        println!("======================================= run {} =======================================", runNumber);
        return test.run();
    }

    fn update(results: &mut Vec<i64>, result: i64) {
        results.drain(0..1);
        results.push(result);
    }

    fn areAllResultsTheSame(results: &Vec<i64>) -> bool {
        let oldestResult = results[0];

        for i in 1..results.len() {
            let result = results[i];

            if result != oldestResult {
                return false;
            }
        }

        return true;
    }
}