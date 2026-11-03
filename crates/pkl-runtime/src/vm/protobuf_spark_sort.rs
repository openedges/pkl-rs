/*
 * Based on TimSort.java from the Android Open Source Project
 *
 *  Copyright (C) 2008 The Android Open Source Project
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *       http://www.apache.org/licenses/LICENSE-2.0
 *
 *  Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 */
// New Rust index-buffer adaptation from Apache Spark v3.5.7 TimSort.java:
// commit ed00d046951a7ecda6429accd3b9c5b2dc792b65,
// blob 863e2e213e7035335916a17f0aa96884b2a0fd41,
// SHA-256 8b729d5f6bdb4ad02aac46264024eb7af6a167bb4751dccac3c8dd8727670290.
// Spark's SortDataFormat operations specialize to copying original indices.
// The comparator is intentionally not assumed reflexive, transitive or total.
const MIN_MERGE: usize = 32;
const MIN_GALLOP: usize = 7;
const CONTRACT_ERROR: &str = "Comparison method violates its general contract!";

pub(super) fn order<F: Fn(usize, usize) -> i32>(
    length: usize,
    comparator: F,
) -> Result<Vec<usize>, &'static str> {
    if length > i32::MAX as usize {
        return Err("union exceeds Java index range");
    }
    let mut data: Vec<_> = (0..length).collect();
    if length < 2 {
        return Ok(data);
    }
    if length < MIN_MERGE {
        let sorted = count_run(&mut data, 0, length, &comparator);
        binary_sort(&mut data, 0, length, sorted, &comparator);
        return Ok(data);
    }
    let minimum = min_run_length(length);
    let mut state = SortState {
        data,
        comparator,
        pending: Vec::new(),
        temporary: vec![0; if length < 512 { length / 2 } else { 256 }],
        minimum_gallop: MIN_GALLOP as i32,
    };
    let mut position = 0;
    while position < length {
        let mut run_length = count_run(&mut state.data, position, length, &state.comparator);
        if run_length < minimum {
            let extended = minimum.min(length - position);
            binary_sort(
                &mut state.data,
                position,
                position + extended,
                position + run_length,
                &state.comparator,
            );
            run_length = extended;
        }
        state.pending.push(Run {
            base: position,
            length: run_length,
        });
        state.merge_collapse()?;
        position += run_length;
    }
    state.merge_force_collapse()?;
    Ok(state.data)
}

fn min_run_length(mut length: usize) -> usize {
    let mut low_bits = 0;
    while length >= MIN_MERGE {
        low_bits |= length & 1;
        length >>= 1;
    }
    length + low_bits
}
fn binary_sort<F: Fn(usize, usize) -> i32>(
    data: &mut [usize],
    lower: usize,
    upper: usize,
    first_unsorted: usize,
    comparator: &F,
) {
    for index in first_unsorted.max(lower + 1)..upper {
        let pivot = data[index];
        let (mut left, mut right) = (lower, index);
        while left != right {
            let middle = (left + right) >> 1;
            if comparator(pivot, data[middle]) < 0 {
                right = middle;
            } else {
                left = middle + 1;
            }
        }
        data.copy_within(left..index, left + 1);
        data[left] = pivot;
    }
}
fn count_run<F: Fn(usize, usize) -> i32>(
    data: &mut [usize],
    lower: usize,
    upper: usize,
    comparator: &F,
) -> usize {
    if lower + 1 == upper {
        return 1;
    }
    let descending = comparator(data[lower + 1], data[lower]) < 0;
    let mut end = lower + 2;
    if descending {
        while end < upper && comparator(data[end], data[end - 1]) < 0 {
            end += 1;
        }
        data[lower..end].reverse();
    } else {
        while end < upper && comparator(data[end], data[end - 1]) >= 0 {
            end += 1;
        }
    }
    end - lower
}
#[derive(Clone, Copy)]
struct Run {
    base: usize,
    length: usize,
}
struct SortState<F> {
    data: Vec<usize>,
    comparator: F,
    pending: Vec<Run>,
    temporary: Vec<usize>,
    minimum_gallop: i32,
}
impl<F: Fn(usize, usize) -> i32> SortState<F> {
    fn merge_collapse(&mut self) -> Result<(), &'static str> {
        while self.pending.len() > 1 {
            let mut selected = self.pending.len() - 2;
            let lengths = &self.pending;
            if (selected > 0
                && lengths[selected - 1].length
                    <= lengths[selected].length + lengths[selected + 1].length)
                || (selected > 1
                    && lengths[selected - 2].length
                        <= lengths[selected].length + lengths[selected - 1].length)
            {
                if lengths[selected - 1].length < lengths[selected + 1].length {
                    selected -= 1;
                }
            } else if lengths[selected].length > lengths[selected + 1].length {
                break;
            }
            self.merge_at(selected)?;
        }
        Ok(())
    }
    fn merge_force_collapse(&mut self) -> Result<(), &'static str> {
        while self.pending.len() > 1 {
            let mut selected = self.pending.len() - 2;
            if selected > 0 && self.pending[selected - 1].length < self.pending[selected + 1].length
            {
                selected -= 1;
            }
            self.merge_at(selected)?;
        }
        Ok(())
    }
    fn merge_at(&mut self, index: usize) -> Result<(), &'static str> {
        let mut first = self.pending[index];
        let mut second = self.pending[index + 1];
        self.pending[index].length += second.length;
        self.pending.remove(index + 1);
        let skipped = gallop_right(
            self.data[second.base],
            &self.data,
            first.base,
            first.length,
            0,
            &self.comparator,
        );
        first.base += skipped;
        first.length -= skipped;
        if first.length == 0 {
            return Ok(());
        }
        second.length = gallop_left(
            self.data[first.base + first.length - 1],
            &self.data,
            second.base,
            second.length,
            second.length - 1,
            &self.comparator,
        );
        if second.length == 0 {
            return Ok(());
        }
        if first.length <= second.length {
            self.merge_low(first, second)
        } else {
            self.merge_high(first, second)
        }
    }
    fn ensure_capacity(&mut self, minimum: usize) {
        if self.temporary.len() >= minimum {
            return;
        }
        // Spark chooses the next strictly larger power of two, capped at half
        // the input. Only the initialized index buffer is retained between merges.
        let next = minimum
            .checked_next_power_of_two()
            .map(|value| if value == minimum { value * 2 } else { value })
            .unwrap_or(minimum);
        self.temporary
            .resize(next.min(self.data.len() / 2).max(minimum), 0);
    }
    fn merge_low(&mut self, first: Run, second: Run) -> Result<(), &'static str> {
        self.ensure_capacity(first.length);
        let (a, temporary, comparator) = (&mut self.data, &mut self.temporary, &self.comparator);
        temporary[..first.length].copy_from_slice(&a[first.base..first.base + first.length]);
        let (mut left, mut right, mut write) = (0, second.base, first.base);
        let (mut remaining_left, mut remaining_right) = (first.length, second.length);
        a[write] = a[right];
        write += 1;
        right += 1;
        remaining_right -= 1;
        if remaining_right == 0 {
            a[write..write + remaining_left].copy_from_slice(&temporary[..remaining_left]);
            return Ok(());
        }
        if remaining_left == 1 {
            a.copy_within(right..right + remaining_right, write);
            a[write + remaining_right] = temporary[left];
            return Ok(());
        }
        let mut threshold = self.minimum_gallop;
        'merge: loop {
            let (mut left_wins, mut right_wins) = (0usize, 0usize);
            loop {
                if comparator(a[right], temporary[left]) < 0 {
                    a[write] = a[right];
                    write += 1;
                    right += 1;
                    right_wins += 1;
                    left_wins = 0;
                    remaining_right -= 1;
                    if remaining_right == 0 {
                        break 'merge;
                    }
                } else {
                    a[write] = temporary[left];
                    write += 1;
                    left += 1;
                    left_wins += 1;
                    right_wins = 0;
                    remaining_left -= 1;
                    if remaining_left == 1 {
                        break 'merge;
                    }
                }
                if (left_wins | right_wins) as i32 >= threshold {
                    break;
                }
            }
            loop {
                left_wins = gallop_right(a[right], temporary, left, remaining_left, 0, comparator);
                if left_wins > 0 {
                    a[write..write + left_wins].copy_from_slice(&temporary[left..left + left_wins]);
                    write += left_wins;
                    left += left_wins;
                    remaining_left -= left_wins;
                    if remaining_left <= 1 {
                        break 'merge;
                    }
                }
                a[write] = a[right];
                write += 1;
                right += 1;
                remaining_right -= 1;
                if remaining_right == 0 {
                    break 'merge;
                }
                right_wins = gallop_left(temporary[left], a, right, remaining_right, 0, comparator);
                if right_wins > 0 {
                    a.copy_within(right..right + right_wins, write);
                    write += right_wins;
                    right += right_wins;
                    remaining_right -= right_wins;
                    if remaining_right == 0 {
                        break 'merge;
                    }
                }
                a[write] = temporary[left];
                write += 1;
                left += 1;
                remaining_left -= 1;
                if remaining_left == 1 {
                    break 'merge;
                }
                threshold -= 1;
                if left_wins < MIN_GALLOP && right_wins < MIN_GALLOP {
                    break;
                }
            }
            threshold = threshold.max(0) + 2;
        }
        self.minimum_gallop = threshold.max(1);
        match remaining_left {
            0 => Err(CONTRACT_ERROR),
            1 => {
                a.copy_within(right..right + remaining_right, write);
                a[write + remaining_right] = temporary[left];
                Ok(())
            }
            count => {
                a[write..write + count].copy_from_slice(&temporary[left..left + count]);
                Ok(())
            }
        }
    }
    fn merge_high(&mut self, first: Run, second: Run) -> Result<(), &'static str> {
        self.ensure_capacity(second.length);
        let (a, temporary, comparator) = (&mut self.data, &mut self.temporary, &self.comparator);
        temporary[..second.length].copy_from_slice(&a[second.base..second.base + second.length]);
        // Cursors point one past the element to move, avoiding negative sentinel
        // indices while preserving Spark's right-to-left comparison/move order.
        let (mut left, mut right, mut write) = (
            first.base + first.length,
            second.length,
            second.base + second.length,
        );
        let (mut remaining_left, mut remaining_right) = (first.length, second.length);
        write -= 1;
        left -= 1;
        a[write] = a[left];
        remaining_left -= 1;
        if remaining_left == 0 {
            a[write - remaining_right..write].copy_from_slice(&temporary[..remaining_right]);
            return Ok(());
        }
        if remaining_right == 1 {
            left -= remaining_left;
            write -= remaining_left;
            a.copy_within(left..left + remaining_left, write);
            a[write - 1] = temporary[right - 1];
            return Ok(());
        }
        let mut threshold = self.minimum_gallop;
        'merge: loop {
            let (mut left_wins, mut right_wins) = (0usize, 0usize);
            loop {
                if comparator(temporary[right - 1], a[left - 1]) < 0 {
                    write -= 1;
                    left -= 1;
                    a[write] = a[left];
                    left_wins += 1;
                    right_wins = 0;
                    remaining_left -= 1;
                    if remaining_left == 0 {
                        break 'merge;
                    }
                } else {
                    write -= 1;
                    right -= 1;
                    a[write] = temporary[right];
                    right_wins += 1;
                    left_wins = 0;
                    remaining_right -= 1;
                    if remaining_right == 1 {
                        break 'merge;
                    }
                }
                if (left_wins | right_wins) as i32 >= threshold {
                    break;
                }
            }
            loop {
                left_wins = remaining_left
                    - gallop_right(
                        temporary[right - 1],
                        a,
                        first.base,
                        remaining_left,
                        remaining_left - 1,
                        comparator,
                    );
                if left_wins > 0 {
                    write -= left_wins;
                    left -= left_wins;
                    remaining_left -= left_wins;
                    a.copy_within(left..left + left_wins, write);
                    if remaining_left == 0 {
                        break 'merge;
                    }
                }
                write -= 1;
                right -= 1;
                a[write] = temporary[right];
                remaining_right -= 1;
                if remaining_right == 1 {
                    break 'merge;
                }
                right_wins = remaining_right
                    - gallop_left(
                        a[left - 1],
                        temporary,
                        0,
                        remaining_right,
                        remaining_right - 1,
                        comparator,
                    );
                if right_wins > 0 {
                    write -= right_wins;
                    right -= right_wins;
                    remaining_right -= right_wins;
                    a[write..write + right_wins]
                        .copy_from_slice(&temporary[right..right + right_wins]);
                    if remaining_right <= 1 {
                        break 'merge;
                    }
                }
                write -= 1;
                left -= 1;
                a[write] = a[left];
                remaining_left -= 1;
                if remaining_left == 0 {
                    break 'merge;
                }
                threshold -= 1;
                if left_wins < MIN_GALLOP && right_wins < MIN_GALLOP {
                    break;
                }
            }
            threshold = threshold.max(0) + 2;
        }
        self.minimum_gallop = threshold.max(1);
        match remaining_right {
            0 => Err(CONTRACT_ERROR),
            1 => {
                left -= remaining_left;
                write -= remaining_left;
                a.copy_within(left..left + remaining_left, write);
                a[write - 1] = temporary[right - 1];
                Ok(())
            }
            count => {
                a[write - count..write].copy_from_slice(&temporary[..count]);
                Ok(())
            }
        }
    }
}

fn gallop_left<F: Fn(usize, usize) -> i32>(
    key: usize,
    a: &[usize],
    base: usize,
    length: usize,
    hint: usize,
    c: &F,
) -> usize {
    let (mut last, mut offset) = (0i64, 1i64);
    if c(key, a[base + hint]) > 0 {
        let maximum = (length - hint) as i64;
        while offset < maximum && c(key, a[base + hint + offset as usize]) > 0 {
            last = offset;
            offset = (offset * 2 + 1).min(maximum);
        }
        offset = offset.min(maximum);
        last += hint as i64;
        offset += hint as i64;
    } else {
        let maximum = (hint + 1) as i64;
        while offset < maximum && c(key, a[base + hint - offset as usize]) <= 0 {
            last = offset;
            offset = (offset * 2 + 1).min(maximum);
        }
        offset = offset.min(maximum);
        let saved = last;
        last = hint as i64 - offset;
        offset = hint as i64 - saved;
    }
    last += 1;
    while last < offset {
        let middle = last + ((offset - last) >> 1);
        if c(key, a[base + middle as usize]) > 0 {
            last = middle + 1;
        } else {
            offset = middle;
        }
    }
    offset as usize
}
fn gallop_right<F: Fn(usize, usize) -> i32>(
    key: usize,
    a: &[usize],
    base: usize,
    length: usize,
    hint: usize,
    c: &F,
) -> usize {
    let (mut offset, mut last) = (1i64, 0i64);
    if c(key, a[base + hint]) < 0 {
        let maximum = (hint + 1) as i64;
        while offset < maximum && c(key, a[base + hint - offset as usize]) < 0 {
            last = offset;
            offset = (offset * 2 + 1).min(maximum);
        }
        offset = offset.min(maximum);
        let saved = last;
        last = hint as i64 - offset;
        offset = hint as i64 - saved;
    } else {
        let maximum = (length - hint) as i64;
        while offset < maximum && c(key, a[base + hint + offset as usize]) >= 0 {
            last = offset;
            offset = (offset * 2 + 1).min(maximum);
        }
        offset = offset.min(maximum);
        last += hint as i64;
        offset += hint as i64;
    }
    last += 1;
    while last < offset {
        let middle = last + ((offset - last) >> 1);
        if c(key, a[base + middle as usize]) < 0 {
            offset = middle;
        } else {
            last = middle + 1;
        }
    }
    offset as usize
}
