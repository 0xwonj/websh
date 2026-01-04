//! A fixed-capacity ring buffer (circular buffer) for O(1) push operations.

use std::iter::FusedIterator;

// ============================================================================
// RingBuffer
// ============================================================================

/// A fixed-capacity circular buffer with O(1) push operations.
///
/// When the buffer reaches capacity, new elements overwrite the oldest ones.
#[derive(Clone)]
pub struct RingBuffer<T> {
    data: Vec<Option<T>>,
    head: usize,
    len: usize,
    capacity: usize,
}

impl<T> RingBuffer<T> {
    /// Creates a new ring buffer with the specified capacity.
    ///
    /// # Panics
    ///
    /// Panics if `capacity` is zero.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "RingBuffer capacity must be greater than 0");

        Self {
            data: (0..capacity).map(|_| None).collect(),
            head: 0,
            len: 0,
            capacity,
        }
    }

    /// Adds an element to the back of the buffer. O(1).
    ///
    /// If at capacity, the oldest element is overwritten.
    pub fn push(&mut self, item: T) {
        let insert_index = (self.head + self.len) % self.capacity;
        self.data[insert_index] = Some(item);

        if self.len == self.capacity {
            self.head = (self.head + 1) % self.capacity;
        } else {
            self.len += 1;
        }
    }

    /// Extends the buffer with elements from an iterator.
    pub fn extend(&mut self, iter: impl IntoIterator<Item = T>) {
        for item in iter {
            self.push(item);
        }
    }

    /// Returns a reference to the element at the given logical index.
    ///
    /// Index 0 is the oldest element, index `len - 1` is the newest.
    #[inline]
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        let actual_index = (self.head + index) % self.capacity;
        self.data[actual_index].as_ref()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Clears all elements from the buffer.
    pub fn clear(&mut self) {
        for slot in &mut self.data {
            *slot = None;
        }
        self.head = 0;
        self.len = 0;
    }

    /// Returns an iterator over references to the elements (oldest to newest).
    pub fn iter(&self) -> Iter<'_, T> {
        Iter {
            buffer: self,
            front: 0,
            back: self.len,
        }
    }

    /// Collects all elements into a `Vec`.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.iter().cloned().collect()
    }
}

impl<T> Default for RingBuffer<T> {
    fn default() -> Self {
        Self::new(1)
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for RingBuffer<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RingBuffer")
            .field("len", &self.len)
            .field("capacity", &self.capacity)
            .field("elements", &self.iter().collect::<Vec<_>>())
            .finish()
    }
}

// ============================================================================
// Iterator Implementation
// ============================================================================

/// An iterator over references to elements in a `RingBuffer`.
pub struct Iter<'a, T> {
    buffer: &'a RingBuffer<T>,
    front: usize,
    back: usize,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let item = self.buffer.get(self.front);
        self.front += 1;
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back.saturating_sub(self.front);
        (remaining, Some(remaining))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        self.buffer.get(self.back)
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {}
impl<'a, T> FusedIterator for Iter<'a, T> {}

impl<'a, T> IntoIterator for &'a RingBuffer<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ============================================================================
// IntoIterator for owned iteration
// ============================================================================

/// An owning iterator over elements in a `RingBuffer`.
pub struct IntoIter<T> {
    buffer: RingBuffer<T>,
    front: usize,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.buffer.len {
            return None;
        }
        let actual_index = (self.buffer.head + self.front) % self.buffer.capacity;
        self.front += 1;
        self.buffer.data[actual_index].take()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.buffer.len.saturating_sub(self.front);
        (remaining, Some(remaining))
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {}
impl<T> FusedIterator for IntoIter<T> {}

impl<T> IntoIterator for RingBuffer<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            buffer: self,
            front: 0,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_buffer() {
        let buffer: RingBuffer<i32> = RingBuffer::new(5);
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.capacity(), 5);
        assert!(buffer.is_empty());
    }

    #[test]
    #[should_panic(expected = "capacity must be greater than 0")]
    fn test_zero_capacity_panics() {
        let _: RingBuffer<i32> = RingBuffer::new(0);
    }

    #[test]
    fn test_push_within_capacity() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);

        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.get(0), Some(&1));
        assert_eq!(buffer.get(1), Some(&2));
        assert_eq!(buffer.get(2), None);
    }

    #[test]
    fn test_push_overflow() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4);
        buffer.push(5);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.get(0), Some(&3));
        assert_eq!(buffer.get(1), Some(&4));
        assert_eq!(buffer.get(2), Some(&5));
    }

    #[test]
    fn test_extend() {
        let mut buffer = RingBuffer::new(3);
        buffer.extend([1, 2, 3, 4, 5]);

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.to_vec(), vec![3, 4, 5]);
    }

    #[test]
    fn test_clear() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.clear();

        assert!(buffer.is_empty());
        assert_eq!(buffer.len(), 0);
        assert_eq!(buffer.get(0), None);
    }

    #[test]
    fn test_iter() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let items: Vec<_> = buffer.iter().collect();
        assert_eq!(items, vec![&1, &2, &3]);
    }

    #[test]
    fn test_iter_after_overflow() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);
        buffer.push(4);

        let items: Vec<_> = buffer.iter().collect();
        assert_eq!(items, vec![&2, &3, &4]);
    }

    #[test]
    fn test_iter_reverse() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let items: Vec<_> = buffer.iter().rev().collect();
        assert_eq!(items, vec![&3, &2, &1]);
    }

    #[test]
    fn test_into_iter() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(String::from("a"));
        buffer.push(String::from("b"));

        let items: Vec<_> = buffer.into_iter().collect();
        assert_eq!(items, vec!["a", "b"]);
    }

    #[test]
    fn test_into_iter_after_overflow() {
        let mut buffer = RingBuffer::new(2);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let items: Vec<_> = buffer.into_iter().collect();
        assert_eq!(items, vec![2, 3]);
    }

    #[test]
    fn test_exact_size_iterator() {
        let mut buffer = RingBuffer::new(5);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        let iter = buffer.iter();
        assert_eq!(iter.len(), 3);
    }

    #[test]
    fn test_to_vec() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);

        assert_eq!(buffer.to_vec(), vec![1, 2]);
    }

    #[test]
    fn test_clone() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);

        let cloned = buffer.clone();
        assert_eq!(cloned.len(), 2);
        assert_eq!(cloned.get(0), Some(&1));
        assert_eq!(cloned.get(1), Some(&2));
    }

    #[test]
    fn test_single_capacity() {
        let mut buffer = RingBuffer::new(1);
        buffer.push(1);
        buffer.push(2);
        buffer.push(3);

        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer.get(0), Some(&3));
    }

    #[test]
    fn test_wraparound_multiple_times() {
        let mut buffer = RingBuffer::new(3);
        for i in 0..10 {
            buffer.push(i);
        }

        assert_eq!(buffer.len(), 3);
        assert_eq!(buffer.to_vec(), vec![7, 8, 9]);
    }

    #[test]
    fn test_debug_format() {
        let mut buffer = RingBuffer::new(3);
        buffer.push(1);
        buffer.push(2);

        let debug_str = format!("{:?}", buffer);
        assert!(debug_str.contains("RingBuffer"));
        assert!(debug_str.contains("len: 2"));
        assert!(debug_str.contains("capacity: 3"));
    }
}
