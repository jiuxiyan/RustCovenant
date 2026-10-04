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
    pub fn new_uninitialized(size : usize) -> Self {
        if size == 0 {
            return Self {
                length : 0,
                data   : core::ptr::null_mut(),
            };
        }

        // Create a layout for the allocation
        let lay = Layout::array::<T>(size).unwrap();

        // ZSTs don't need (and must not rely on) an allocation.
        if lay.size() == 0 {
            return Self {
                length : size,
                data   : core::ptr::NonNull::<T>::dangling().as_ptr(),
            };
        }

        // Check that the allocation was successful
        let ptr = unsafe { alloc(lay) as *mut T };
        if ptr.is_null() {
            handle_alloc_error(lay);
        }

        Self {
            length : size,
            data   : ptr,
        }
    }

    /// If successful, allocates size * size_of::<T>() bytes into the heap,
    /// and then initializes each byte with 0.
    /// This function will panic in the same conditions alloc::alloc(Layout) will
    /// (so the same conditions for Box<T>, Vec<T>, etc).
    ///
    /// Note: size must be non zero to obtain a valid pointer.
    pub fn new_zeroed(size : usize) -> Self {
        let mut c = Self::new_uninitialized(size);

        // Only touch memory when there's actual allocation backing it.
        let lay = Layout::array::<T>(size).unwrap();
        if lay.size() != 0 {
            unsafe { core::ptr::write_bytes(c.data as *mut u8, 0, lay.size()); }
        }

        c
    }

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
    pub fn new(size : usize, default : T) -> Self {
        let c = Self::new_uninitialized(size);

        for i in 0..size {
            unsafe { c.data.add(i).write(default); }
        }

        c
    }
}

impl<T> Drop for Caja<T> {
    fn drop(&mut self) {
        if self.length == 0 || self.data.is_null() {
            return;
        }

        let lay = Layout::array::<T>(self.length).unwrap();
        if lay.size() == 0 {
            return;
        }

        unsafe {
            dealloc(self.data as *mut u8, lay);
        }
    }
}

impl<T> Index<usize> for Caja<T> {
    type Output = T;

    /// Index into the array.
    ///
    /// Panics if `index` is out of bounds.
    fn index(&self, index : usize) -> &Self::Output {
        if index >= self.length {
            panic!("Caja index out of bounds");
        }

        unsafe { &*self.data.add(index) }
    }
}
impl<T> IndexMut<usize> for Caja<T> {
    /// Index into the array (mutably).
    ///
    /// Panics if `index` is out of bounds.
    fn index_mut(&mut self, index : usize) -> &mut Self::Output {
        if index >= self.length {
            panic!("Caja index out of bounds");
        }

        unsafe { &mut *self.data.add(index) }
    }
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
        if self.length == 0 {
            return Self {
                length : 0,
                data   : core::ptr::null_mut(),
            };
        }

        // Create a layout for the allocation
        let lay = Layout::array::<T>(self.length).unwrap();

        // ZSTs don't need (and must not rely on) an allocation.
        if lay.size() == 0 {
            return Self {
                length : self.length,
                data   : core::ptr::NonNull::<T>::dangling().as_ptr(),
            };
        }

        // Check that the allocation was successful
        let ptr = unsafe { alloc(lay) as *mut T };
        if ptr.is_null() {
            handle_alloc_error(lay);
        }

        for i in 0..self.length {
            unsafe { ptr.add(i).write(*self.data.add(i)); };
        }

        Self {
            length : self.length,
            data   : ptr,
        }
    }
}

impl<T : Display> Display for Caja<T> {
    fn fmt(&self, format : &mut Formatter<'_>) -> fmt::Result {
        write!(format, "Length : {}\nData : [", self.length)?;

        if self.length == 0 {
            return write!(format, " ]\n");
        }

        write!(format, " ")?;
        for i in 0..self.length {
            if i != 0 {
                write!(format, ", ")?;
            }
            write!(format, "{}", self[i])?;
        }
        write!(format, " ]\n")
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
