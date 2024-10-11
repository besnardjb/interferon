use std::{fs::File, iter};

use anyhow::Result;
use histo::Histogram;
use rand::Rng;
use rayon::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug)]
/// Represents a periodic gate function.
struct Gate {
    /// The duration of the high pulse in seconds.
    high_duration: f64,
    /// The duration of the low pulse in seconds.
    low_duration: f64,
    /// Start time for the start of the first low period.
    start_time: f64,
}

impl Gate {
    /// Generate the gate signal at the given time t.
    fn calculate_value(&self, t: f64) -> f64 {
        let period = self.period();
        let phase = (t - self.start_time).rem_euclid(period).abs();

        if phase < self.high_duration {
            1.0
        } else {
            0.0
        }
    }

    /// Creates a new `Gate` with randomly generated parameters.
    ///
    /// The high and low durations are equal, and the start time is a random value between 0 and the high duration.
    fn new_random_periodic(max_period: f64, high_ratio: f64) -> Gate {
        let period = rand::thread_rng().gen_range(10..max_period as i32) as f64;

        let low_duration = (1.0 - high_ratio) * period;

        let high_duration = period - low_duration;

        // Generate a random start time between 0 and the high duration
        let start_time = rand::random::<f64>() * period;

        Gate {
            high_duration: high_duration.ceil(),
            low_duration: low_duration.ceil(),
            start_time: start_time.ceil(),
        }
    }

    fn randomize_start_time(gates: &mut [Gate]) {
        for g in gates {
            g.start_time = (rand::thread_rng().gen_range(0..100000) as f64 * g.period()) / 100000.0;
        }
    }

    fn generate_n_periodic(n: i64, max_period: f64, high_ratio: f64) -> Vec<Gate> {
        (0..n)
            .map(|_| Gate::new_random_periodic(max_period, high_ratio))
            .collect()
    }

    fn period(&self) -> f64 {
        self.low_duration + self.high_duration
    }

    fn max_period(gates: &[Gate]) -> f64 {
        gates
            .par_iter()
            .map(|v| v.period())
            .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less))
            .unwrap_or(0.0)
    }

    fn evaluate_max_on_range(gates: &[Gate], points: &[f64]) -> f64 {
        let max: Option<f64> = points
            .par_iter()
            .map(|tt| gates.iter().map(|v| v.calculate_value(*tt)).sum::<f64>())
            .max_by(|a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Less));

        max.unwrap_or(0.0)
    }
}

#[derive(Serialize)]
struct MyBucket {
    start: u64,
    end: u64,
    count: u64,
}

fn histogram_to_file(hist: &Histogram, file: &str) -> Result<()> {
    let f = File::create(file)?;

    let buckets: Vec<MyBucket> = hist
        .buckets()
        .map(|v| MyBucket {
            start: v.start(),
            end: v.end(),
            count: v.count(),
        })
        .collect();

    serde_json::to_writer_pretty(f, &buckets)?;

    Ok(())
}

const JOB_COUNT: i64 = 1000;
const CONFIG_COUNT: u64 = 50000;
const MAX_PERIOD: f64 = 20.0;
const HIGH_RATIO: f64 = 0.5;

fn run_gates(gates: Vec<Gate>, target: &str) -> Result<()> {
    let mut gates = gates;
    let max_period = Gate::max_period(&gates);

    println!("Job count : {}", JOB_COUNT);
    println!("Number of runs : {}", CONFIG_COUNT);
    println!("IO Ratio : {}", HIGH_RATIO);


    let mut histogram = Histogram::with_buckets(20);

    let bar = indicatif::ProgressBar::new(CONFIG_COUNT);
    /* This is the random jobs together */

    let mut t = max_period;

    let mut points: Vec<f64> = Vec::new();

    loop {
        if t >= max_period * 2.0 {
            break;
        }
        points.push(t);

        t += 0.5;
    }

    for _ in 0..CONFIG_COUNT {
        Gate::randomize_start_time(&mut gates);
        let max_val = Gate::evaluate_max_on_range(&gates, &points);
        histogram.add(max_val as u64);
        bar.inc(1);
    }

    bar.finish();

    println!("{}", histogram);

    histogram_to_file(&histogram, target)?;

    Ok(())
}

fn run_random(count: i64) -> Result<Vec<Gate>> {
    let gates = Gate::generate_n_periodic(count, MAX_PERIOD, HIGH_RATIO);

    run_gates(gates.clone(), "./random.json")?;

    Ok(gates)
}

fn run_with_coherency(gates: Vec<Gate>, percentage: f64, groups_count: u32) -> Result<()> {
    println!("#######################################");
    println!("PCT {} GROUPS {}", percentage, groups_count);
    println!("#######################################");

    let mut gates = gates;

    let to_drop = (gates.len() as f64 * percentage) as usize;

    gates.truncate(gates.len() - to_drop);

    let mut per_group = to_drop / groups_count as usize;

    if per_group == 0 {
        per_group = 1;
    }

    for _ in 0..groups_count {
        let wave = Gate::new_random_periodic(MAX_PERIOD, HIGH_RATIO);
        let mut coherent_waves: Vec<Gate> = iter::repeat(wave.clone()).take(per_group).collect();
        gates.append(&mut coherent_waves);
    }

    run_gates(
        gates,
        &format!("./pct_{}_groups_{}.json", percentage, groups_count),
    )?;

    Ok(())
}

fn main() -> Result<()> {
    let gates = run_random(JOB_COUNT)?;

    for percentage in [0.1, 0.2, 0.5, 1.0] {
        for groups in [1] {
            run_with_coherency(gates.clone(), percentage, groups)?;
        }
    }

    Ok(())
}
