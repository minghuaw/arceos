//! Talc allocation

use crate::{AllocError, AllocResult, BaseAllocator, ByteAllocator};

use talc::{ErrOnOom, Span, Talc, Talck};

pub struct YourNewAllocator {
    talc: Talck<spin::Mutex<()>, ErrOnOom>,
    claimed_span: Option<Span>,
    used_bytes: usize,
}

impl YourNewAllocator {
    pub const fn new() -> Self {
        Self {
            talc: Talc::new(ErrOnOom).lock::<spin::Mutex<()>>(),
            claimed_span: None,
            used_bytes: 0,
        }
    }
}

impl BaseAllocator for YourNewAllocator {
    fn init(&mut self, start: usize, size: usize) {
        let span_to_claim = Span::from_base_size(start as *mut u8, size);
        let span = unsafe {
             self.talc.lock().claim(span_to_claim).unwrap() // Simply panic if the claim fails
        };
        self.claimed_span = Some(span);
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        let span_to_claim = Span::from_base_size(start as *mut u8, size);

        match self.claimed_span {
            Some(old_heap) => {
                let req_heap = old_heap.clone().fit_over(span_to_claim);
                let new_span = unsafe {
                    self.talc.lock().extend(old_heap, req_heap)
                };
                self.claimed_span = Some(new_span);
                Ok(())
            },
            None => {
                let span = unsafe {
                    self.talc.lock().claim(span_to_claim).unwrap() // Simply panic if the claim fails
                };
                self.claimed_span = Some(span);
                Ok(())
            }
        }
    }
}

impl ByteAllocator for YourNewAllocator {
    fn alloc(&mut self, layout: core::alloc::Layout) -> AllocResult<core::ptr::NonNull<u8>> {
        let ptr = unsafe { self.talc.lock().malloc(layout).map_err(|_| AllocError::NoMemory) }?;
        self.used_bytes = self.used_bytes.checked_add(layout.size()).ok_or(AllocError::MemoryOverlap)?;
        Ok(ptr)
    }

    fn dealloc(&mut self, pos: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        unsafe { self.talc.lock().free(pos, layout) }
        self.used_bytes = self.used_bytes.saturating_sub(layout.size());
    }

    fn total_bytes(&self) -> usize {
        self.claimed_span.as_ref().map(|s| s.size()).unwrap_or(0)
    }

    fn used_bytes(&self) -> usize {
        self.used_bytes
    }

    fn available_bytes(&self) -> usize {
        self.total_bytes() - self.used_bytes()
    }
}