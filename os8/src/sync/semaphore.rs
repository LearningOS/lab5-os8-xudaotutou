use crate::sync::UPSafeCell;
use crate::task::{add_task, block_current_and_run_next, current_task, TaskControlBlock, current_process};
use alloc::{collections::VecDeque, sync::Arc};

pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(res_count: usize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
        }
    }

    pub fn up(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if let Some(task) = current_task() {
            let task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.as_ref() {
                let tid = res.tid;
                let process = current_process();
                let mut process_inner = process.inner_exclusive_access();
                let sem_id = process_inner.semaphore_id;
                process_inner.semaphore_available_vector[sem_id] -= 1;
                process_inner.semaphore_allocation_vector[tid][sem_id] += 1;
            }
        }
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                add_task(task);
            }
        }
    }

    pub fn down(&self) {
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if let Some(task) = current_task() {
            let task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.as_ref() {
                let tid = res.tid;
                let process = current_process();
                let mut process_inner = process.inner_exclusive_access();
                let sem_id = process_inner.semaphore_id;
                process_inner.semaphore_need_vector[tid][sem_id] -= 1;
                process_inner.semaphore_available_vector[sem_id] += 1;
                process_inner.semaphore_allocation_vector[tid][sem_id] -= 1;
            }
        }
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        }
    }
}