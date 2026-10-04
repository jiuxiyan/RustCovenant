// I didn't like how the standard methods for
// allocating arrays in the heap were lacking
// (Vecs assume that they are going to change
// in size, and Boxes need to be of known
// size at compile time). 
// 
// So I wrote this simple library.

#![allow(dead_code)]
#![no_std]

extern crate alloc;

use alloc::{
    alloc::{alloc, Layout, dealloc, handle_alloc_error},
    fmt, fmt::{Debug, Display, Formatter},
    slice::{self, },
};

use core::{
    mem::size_of, 
    ops::{Index, IndexMut}
};

/// An fixed-sized array allocated in the heap
/// of any arbitrary type, and whose
/// size needn't be known at compile time.
pub struct Caja<T> {
    /// Length of the array
    length : usize,
    data   : *mut T,
}
impl<T> Caja<T> {
    /// If successful, allocates size * size_of::<T>() bytes into the heap,
    /// resulting in an uninitialized array that is used by Caja.
    /// This function will panic in the same conditions alloc::alloc(Layout) will
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub fn new_uninitialized(size : usize) -> Self {unsafe{
        if size == 0 {
            return Self {
                length : 0,
                data   : core::ptr::null_mut()
            };
        }

        // Create a layout for the allocation
        let lay = Layout::array::<T>(size).unwrap();

        // Check that the allocation was successful
        let ptr = alloc(lay) as *mut T;
        if ptr.is_null() {
            handle_alloc_error(lay);
        }

        return Self {
            length : size,
            data   : ptr,
        };
    };}

    /// If successful, allocates size * size_of::<T>() bytes into the heap,
    /// and then initializes each byte with 0.
    /// This function will panic in the same conditions alloc::alloc(Layout) will
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub fn new_zeroed(size : usize) -> Self {unsafe{
        let c = Self::new_uninitialized(size);

        for i in 0..size*size_of::<T>() {
            *(c.data as *mut u8).add(i) = 0;
        }

        return c;
    };}

    /// Returns the underlying pointer in Caja
    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        return self.data;
    }

    /// Returns the length of the array
    #[inline(always)]
    pub fn len(&self) -> usize {
        return self.length;
    }

    /// Returns a slice of the array
    pub fn as_slice(&self) -> &[T] {unsafe{
        return slice::from_raw_parts(self.data, self.length);
    };}

    /// Returns a mutable sliice of the array
    pub fn as_mut_slice(&self) -> &mut [T] {unsafe{
        return slice::from_raw_parts_mut(self.data, self.length);
    };}
}
impl<T : Copy> Caja<T> { 
    /// If successful, allocates an array of type 'T' and size 'size' into 
    /// the heap, and initializes each element with 'default'.
    /// T must implement Copy for this to work.
    /// This function will panic in the same conditions alloc::alloc(Layout) will
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub fn new(size : usize, default : T) -> Self {unsafe{
        let c = Self::new_uninitialized(size);

        for i in 0..size {
            *c.data.add(i) = default;
        }

        return c;
    };}
}

impl<T> Drop for Caja<T> {
    fn drop(&mut self) {unsafe{
        dealloc(
            self.data as *mut u8,
            Layout::array::<T>(self.length).unwrap()
        );
    };}
}

impl<T> Index<usize> for Caja<T> {
    type Output = T;

    ///  Index into the array. This function does NOT do bounds checking
    fn index(&self, index : usize) -> &Self::Output {unsafe{
        return self.data.add(index).as_ref().unwrap();
    };}
}
impl<T> IndexMut<usize> for Caja<T> {
    ///  Index into the array (mutably). This function does NOT do bounds checking
    fn index_mut(&mut self, index : usize) -> &mut Self::Output {unsafe{
        return self.data.add(index).as_mut().unwrap();
    };}
}

impl<T : Copy> From<&[T]> for Caja<T> {
    /// Creates a Caja from a slice, copying the data into
    /// a new buffer in the heap.
    ///
    /// Because this functions creates a  new caja, it will panic 
    /// under the same conditions as the new variations
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    fn from(frm : &[T]) -> Self {
        let mut ret = Self::new_uninitialized(frm.len());
        
        for i in 0..frm.len() {
            ret[i] = frm[i];
        }

        return ret;
    }
}

impl<T : Copy> Clone for Caja<T> {
    /// Clones self, creating a new array on the heap
    /// with the same data as the original one.
    ///    
    /// Because this functions creates a  new caja, it will panic 
    /// under the same conditions as the new variations
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    fn clone(&self) -> Self {
        // Create a layout for the allocation
        let lay = Layout::array::<T>(self.length).unwrap();

        // Check that the allocation was successful
        let ptr = unsafe { alloc(lay) as *mut T };
        if ptr.is_null() {
            handle_alloc_error(lay);
        }

        for i in 0..self.length {
            unsafe { *ptr.add(i) = *self.data.add(i); };
        }

        Self {
            length : self.length,
            data   : ptr,
        }
    }
}

impl<T : Display> Display for Caja<T> {
    fn fmt(&self, format : &mut Formatter<'_>) -> fmt::Result {
        let mut res = write!(format, "Length : {}\nData : [ ", self.length);
        
        for i in 0..self.length-1 {
            if res.is_err() {
                return res;
            }

            res = write!(format, "{}, ", self[i]);
        }
        res = write!(format, "{} ]\n", self[self.length-1]);

        return res;
    }
}

impl<T : Debug> Debug for Caja<T> {
    fn fmt(&self, format : &mut Formatter<'_>) -> fmt::Result {unsafe{
        return format.debug_struct("Caja")
            .field("length", &self.length)
            .field("data", &self.data)
            .field("data as an array", &slice::from_raw_parts(self.data, self.length))
            .finish();
    };}
}
