use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;

pub fn sys_sleep(ms: usize) -> isize {
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}

// LAB5 HINT: you might need to maintain data structures used for deadlock detection
// during sys_mutex_* and sys_semaphore_* syscalls
pub fn sys_mutex_create(blocking: bool) -> isize {
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        process_inner.mutex_available_vector[id] = 1;
        // 每个thread增加一个资源类别
        process_inner
            .mutex_allocation_vector
            .iter_mut()
            .for_each(|resource| resource[id] = 0);
        process_inner
            .mutex_need_vector
            .iter_mut()
            .for_each(|resource| resource[id] = 0);
        process_inner.mutex_id = id;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_available_vector.push(1);
        process_inner
            .mutex_allocation_vector
            .iter_mut()
            .for_each(|resource| resource.push(0));
        process_inner
            .mutex_need_vector
            .iter_mut()
            .for_each(|resource| resource.push(0));
        process_inner.mutex_id = process_inner.mutex_list.len() - 1;
        process_inner.mutex_id as isize
    }
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let mutex = {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();

        if let Some(task) = current_task() {
            let task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.as_ref() {
                let tid = res.tid;
                process_inner.mutex_need_vector[tid][mutex_id] += 1;
                process_inner.mutex_id = mutex_id;
            } else {
                // 没有res,need
                return -1;
            }
        } else {
            // 没有task
            return -1;
        }
        if process_inner.enable_deadlock && process_inner.deadlock_detect() {
            return -0xDEAD;
        }
        Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap())
    };
    mutex.lock();
    return 0;
}

pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    let mutex = {
        let process = current_process();
        let process_inner = process.inner_exclusive_access();
        Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap())
    };
    mutex.unlock();
    0
}

pub fn sys_semaphore_create(res_count: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        process_inner.semaphore_available_vector[id] = res_count;
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner
            .semaphore_allocation_vector
            .iter_mut()
            .for_each(|resource| resource[id] = 0);
        process_inner
            .semaphore_need_vector
            .iter_mut()
            .for_each(|resource| resource[id] = 0);
        process_inner.semaphore_available_vector[id] = 0;
        process_inner.semaphore_id = id;
        id
    } else {
        println!("push!");
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner
            .semaphore_allocation_vector
            .iter_mut()
            .for_each(|resource| resource.push(0));
        process_inner
            .semaphore_need_vector
            .iter_mut()
            .for_each(|resource| resource.push(0));
        process_inner.semaphore_available_vector.push(res_count);
        process_inner.semaphore_id = process_inner.semaphore_list.len() - 1;
        process_inner.semaphore_id
    };
    // 每个thread增加一个资源类别
    println!(
        "avaliable:{:?}, allocation:{:?}, need:{:?}",
        process_inner.semaphore_available_vector,
        process_inner.semaphore_allocation_vector,
        process_inner.semaphore_need_vector
    );

    id as isize
}

pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let sem = {
        let process = current_process();
        let process_inner = process.inner_exclusive_access();

        Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap())
    };
    sem.up();
    0
}

// LAB5 HINT: Return -0xDEAD if deadlock is detected
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let sem = {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        if let Some(task) = current_task() {
            let task_inner = task.inner_exclusive_access();
            if let Some(res) = task_inner.res.as_ref() {
                let tid = res.tid;
                // println!("before need_vector:{:?}",process_inner.semaphore_need_vector);
                process_inner.semaphore_need_vector[tid][sem_id] += 1;
                process_inner.semaphore_id = sem_id;
                // println!("after need_vector:{:?}",process_inner.semaphore_need_vector);
            } else {
                // 没有res,need
                return -1;
            }
        } else {
            // 没有task
            return -1;
        }
        if process_inner.enable_deadlock && process_inner.deadlock_detect() {
            return -0xDEAD;
        }
        Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap())
    };
    sem.down();
    0
}

pub fn sys_condvar_create(_arg: usize) -> isize {
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}

pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}

pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}

// LAB5 YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    let process = current_process();
    match _enabled {
        1 => {
            process.inner_exclusive_access().enable_deadlock = true;
            0
        }
        0 => {
            process.inner_exclusive_access().enable_deadlock = false;
            0
        }
        _ => -1,
    }
}
