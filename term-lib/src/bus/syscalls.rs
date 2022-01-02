use crate::wasmer::Array;
use crate::wasmer::WasmPtr;
use cooked_waker::*;
use std::collections::HashMap;
use std::future::Future;
use std::task::Context;
use std::task::Poll;
use std::task::Waker;
#[allow(unused_imports, dead_code)]
use tracing::{debug, error, info, trace, warn};
use wasm_bus::abi::CallError;
use wasm_bus::abi::CallHandle;

use super::thread::WasmBusThread;
use super::*;

pub(crate) mod raw {
    use super::*;
    pub fn wasm_bus_drop(thread: &WasmBusThread, handle: u32) {
        unsafe { super::wasm_bus_drop(thread, handle.into()) }
    }
    pub fn wasm_bus_handle(thread: &WasmBusThread) -> u32 {
        unsafe { super::wasm_bus_handle(thread).into() }
    }
    pub fn wasm_bus_wake(thread: &WasmBusThread) {
        unsafe { super::wasm_bus_wake(thread) }
    }
    pub fn wasm_bus_tick(thread: &WasmBusThread) -> bool {
        unsafe { super::wasm_bus_tick(thread) }
    }
    pub fn wasm_bus_listen(thread: &WasmBusThread, topic_ptr: u32, topic_len: u32) {
        let topic_ptr: WasmPtr<u8, Array> = WasmPtr::new(topic_ptr as u32);
        unsafe { super::wasm_bus_listen(thread, topic_ptr, topic_len as usize) }
    }
    pub fn wasm_bus_callback(
        thread: &WasmBusThread,
        parent: u32,
        handle: u32,
        topic_ptr: u32,
        topic_len: u32,
    ) {
        let parent: Option<CallHandle> = if parent != u32::MAX {
            Some(parent.into())
        } else {
            None
        };
        let handle: CallHandle = handle.into();
        let topic_ptr: WasmPtr<u8, Array> = WasmPtr::new(topic_ptr as u32);
        unsafe { super::wasm_bus_callback(thread, parent, handle, topic_ptr, topic_len as usize) }
    }
    pub fn wasm_bus_fault(thread: &WasmBusThread, handle: u32, error: u32) {
        let handle: CallHandle = handle.into();
        unsafe { super::wasm_bus_fault(thread, handle, error) }
    }
    pub fn wasm_bus_poll(thread: &WasmBusThread) {
        unsafe { super::wasm_bus_poll(thread) }
    }
    pub fn wasm_bus_reply(
        thread: &WasmBusThread,
        handle: u32,
        response_ptr: u32,
        response_len: u32,
    ) {
        let handle: CallHandle = handle.into();
        let response_ptr: WasmPtr<u8, Array> = WasmPtr::new(response_ptr as u32);
        unsafe { super::wasm_bus_reply(thread, handle, response_ptr, response_len as usize) }
    }
    pub fn wasm_bus_reply_callback(
        thread: &WasmBusThread,
        handle: u32,
        topic_ptr: u32,
        topic_len: u32,
        response_ptr: u32,
        response_len: u32,
    ) {
        let handle: CallHandle = handle.into();
        let topic_ptr: WasmPtr<u8, Array> = WasmPtr::new(topic_ptr as u32);
        let response_ptr: WasmPtr<u8, Array> = WasmPtr::new(response_ptr as u32);
        unsafe {
            super::wasm_bus_reply_callback(
                thread,
                handle,
                topic_ptr,
                topic_len as usize,
                response_ptr,
                response_len as usize,
            )
        }
    }
    pub fn wasm_bus_call(
        thread: &WasmBusThread,
        parent: u32,
        handle: u32,
        wapm_ptr: u32,
        wapm_len: u32,
        topic_ptr: u32,
        topic_len: u32,
        request_ptr: u32,
        request_len: u32,
    ) -> u32 {
        let parent: Option<CallHandle> = if parent != u32::MAX {
            Some(parent.into())
        } else {
            None
        };
        let handle: CallHandle = handle.into();
        let wapm_ptr: WasmPtr<u8, Array> = WasmPtr::new(wapm_ptr as u32);
        let topic_ptr: WasmPtr<u8, Array> = WasmPtr::new(topic_ptr as u32);
        let request_ptr: WasmPtr<u8, Array> = WasmPtr::new(request_ptr as u32);
        unsafe {
            super::wasm_bus_call(
                thread,
                parent,
                handle,
                wapm_ptr,
                wapm_len as usize,
                topic_ptr,
                topic_len as usize,
                request_ptr,
                request_len as usize,
            )
        }
    }
    pub fn wasm_bus_thread_id(thread: &WasmBusThread) -> u32 {
        unsafe { super::wasm_bus_thread_id(thread) }
    }
}

// Drops a handle used by calls or callbacks
unsafe fn wasm_bus_drop(thread: &WasmBusThread, handle: CallHandle) {
    let handle: CallHandle = handle.into();
    let mut inner = thread.inner.unwrap();
    inner.invocations.remove(&handle);
    inner.callbacks.remove(&handle);
    inner.factory.close(CallHandle::from(handle));
}

unsafe fn wasm_bus_handle(_thread: &WasmBusThread) -> CallHandle {
    fastrand::u32(..).into()
}

unsafe fn wasm_bus_wake(thread: &WasmBusThread) {
    thread.waker.wake_by_ref();
}

unsafe fn wasm_bus_tick(thread: &WasmBusThread) -> bool {
    // We enter a loop that the waker will keep running
    let waker: Waker = thread.waker.clone().into_waker();
    let mut cx = Context::from_waker(&waker);
    let start_waker = thread.waker.get();
    let mut last_waker = thread.waker.get();
    loop {
        // Take the invocations out of the idle list and process them
        // (we need to do this outside of the thread local lock as
        //  otherwise the re-entrance will panic the system)
        let invocations = {
            let mut inner = thread.inner.unwrap();
            inner.invocations.drain().collect::<Vec<_>>()
        };

        // Run all the invocations and build a carry over list
        let mut carry_over = Vec::new();
        for (key, mut invocation) in invocations {
            let pinned_invocation = invocation.as_mut();
            if let Poll::Pending = pinned_invocation.poll(&mut cx) {
                carry_over.push((key, invocation));
            }
        }

        // If there are any carry overs then re-add them
        if carry_over.is_empty() == false {
            let mut inner = thread.inner.unwrap();
            for (key, invoke) in carry_over {
                inner.invocations.insert(key, invoke);
            }
        }

        // Update the waker and continue (or if not woken then we are done)
        let cur_waker = thread.waker.get();
        if last_waker == cur_waker {
            break;
        }
        last_waker = cur_waker;
    }

    return start_waker != last_waker;
}

// Incidates that a call that will be made should invoke a callback
// back to this process under the designated handle.
unsafe fn wasm_bus_callback(
    thread: &WasmBusThread,
    parent: Option<CallHandle>,
    handle: CallHandle,
    topic_ptr: WasmPtr<u8, Array>,
    topic_len: usize,
) {
    let topic = topic_ptr
        .get_utf8_str(thread.memory(), topic_len as u32)
        .unwrap();
    debug!(
        "wasm-bus::recv (parent={:?}, handle={}, topic={})",
        parent, handle.id, topic
    );

    let mut inner = thread.inner.unwrap();
    if let Some(parent) = parent {
        let entry = inner.callbacks.entry(parent).or_default();
        entry.insert(topic.to_string(), handle);
        return;
    }
}

// Polls the operating system for messages which will be returned via
// the 'wasm_bus_start' function call.
unsafe fn wasm_bus_poll(thread: &WasmBusThread) {
    trace!("wasm-bus::poll");

    // If we are polling then let anyone waiting for it know
    if *thread.inner.unwrap().polling.borrow() == false {
        let _ = thread.inner.unwrap().polling.send(true);
    }

    // If the poll is woken then return
    if crate::bus::syscalls::raw::wasm_bus_tick(thread) == true {
        return;
    }

    // Lets wait for some work!
    let work = thread.inner.unwrap().work_rx.recv().ok();
    thread.waker.woken();
    match work {
        Some(WasmBusThreadWork::Call {
            topic,
            parent,
            handle,
            data,
            tx,
        }) => {
            let native_memory = thread.memory_ref();
            let native_malloc = thread.wasm_bus_malloc_ref();
            let native_start = thread.wasm_bus_start_ref();
            if native_memory.is_none() || native_malloc.is_none() || native_start.is_none() {
                let _ = tx.send(Err(CallError::IncorrectAbi));
                return;
            }

            // Check the listening is of the correct type
            if thread.inner.unwrap().listens.contains(&topic) == false {
                debug!("invalid topic - {}", topic);
                let _ = tx.send(Err(CallError::InvalidTopic));
                return;
            }

            // Determine the parent handle
            let parent = parent.map(|a| a.into()).unwrap_or(u32::MAX);

            // Record the handler so that when the call completes it notifies the
            // one who put this work on the queue
            let handle = handle.handle();
            {
                let mut inner = thread.inner.unwrap();
                inner.calls.insert(handle, tx);
            }

            // Invoke the call
            let native_memory = native_memory.unwrap();
            let native_malloc = native_malloc.unwrap();
            let native_start = native_start.unwrap();

            let topic = topic.as_bytes();
            let topic_len = topic.len() as u32;
            let topic_ptr = native_malloc.call(topic_len).unwrap();
            native_memory
                .uint8view_with_byte_offset_and_length(topic_ptr, topic_len)
                .copy_from(&topic[..]);

            let request = &data[..];
            let request_len = request.len() as u32;
            let request_ptr = native_malloc.call(request_len).unwrap();
            native_memory
                .uint8view_with_byte_offset_and_length(request_ptr, request_len)
                .copy_from(&request[..]);

            native_start
                .call(
                    parent,
                    handle.id,
                    topic_ptr,
                    topic_len,
                    request_ptr,
                    request_len,
                )
                .unwrap();
        }
        Some(WasmBusThreadWork::Drop { handle }) => {
            if let Some(native_drop) = thread.wasm_bus_drop_ref() {
                native_drop.call(handle.id).unwrap();
            }
        }
        Some(WasmBusThreadWork::Wake) => {
            debug!("polling loop awoken");
            crate::bus::syscalls::raw::wasm_bus_tick(thread);
        }
        None => {
            debug!("polling loop has exited");
        }
    }
}

// Tells the operating system that this program is ready to respond
// to calls on a particular topic name.
unsafe fn wasm_bus_listen(thread: &WasmBusThread, topic_ptr: WasmPtr<u8, Array>, topic_len: usize) {
    let topic = topic_ptr
        .get_utf8_str(thread.memory(), topic_len as u32)
        .unwrap();
    debug!("wasm-bus::listen (topic={})", topic);

    let mut inner = thread.inner.unwrap();
    inner.listens.insert(topic.to_string());
}

// Indicates that a fault has occured while processing a call
unsafe fn wasm_bus_fault(thread: &WasmBusThread, handle: CallHandle, error: u32) {
    use tokio::sync::mpsc::error::TrySendError;

    debug!("wasm-bus::error (handle={}, error={})", handle.id, error);

    // Grab the sender we will relay this response to
    let error: CallError = error.into();
    if let Some(work) = thread.inner.unwrap().calls.remove(&handle) {
        if let Err(err) = work.try_send(Err(error)) {
            let response = match err {
                TrySendError::Closed(a) => a,
                TrySendError::Full(a) => a,
            };
            thread.system.task_shared(Box::new(move || {
                Box::pin(async move {
                    let _ = work.send(response).await;
                })
            }));
        }
    }
}

// Returns the response of a listen invokation to a program
// from the operating system
unsafe fn wasm_bus_reply(
    thread: &WasmBusThread,
    handle: CallHandle,
    response_ptr: WasmPtr<u8, Array>,
    response_len: usize,
) {
    use tokio::sync::mpsc::error::TrySendError;

    debug!(
        "wasm-bus::reply (handle={}, response={} bytes)",
        handle.id, response_len
    );

    // Grab the data we are sending back
    let response = thread
        .memory()
        .uint8view_with_byte_offset_and_length(response_ptr.offset(), response_len as u32)
        .to_vec();

    // Grab the sender we will relay this response to
    if let Some(work) = thread.inner.unwrap().calls.remove(&handle) {
        if let Err(err) = work.try_send(Ok(response)) {
            let response = match err {
                TrySendError::Closed(a) => a,
                TrySendError::Full(a) => a,
            };
            thread.system.task_shared(Box::new(move || {
                Box::pin(async move {
                    let _ = work.send(response).await;
                })
            }));
        }
    }
}

// Returns the response of a listen callback
unsafe fn wasm_bus_reply_callback(
    thread: &WasmBusThread,
    handle: CallHandle,
    topic_ptr: WasmPtr<u8, Array>,
    topic_len: usize,
    response_ptr: WasmPtr<u8, Array>,
    response_len: usize,
) {
    let topic = topic_ptr
        .get_utf8_str(thread.memory(), topic_len as u32)
        .unwrap()
        .to_string();
    debug!(
        "wasm-bus::reply_callback (handle={}, topic={}, response={} bytes)",
        handle.id, topic, response_len
    );

    // Grab the data we are sending back
    let response = thread
        .memory()
        .uint8view_with_byte_offset_and_length(response_ptr.offset(), response_len as u32)
        .to_vec();

    // Grab the callback this related to
    let callback = thread
        .inner
        .unwrap()
        .callbacks
        .get(&handle)
        .map(|handle| handle.get(&topic))
        .flatten()
        .map(|handle| WasmBusCallback::new(thread, handle.clone()).unwrap());

    // Grab the sender we will relay this response to
    if let Some(callback) = callback {
        callback.feed_bytes(response);
    } else {
        debug!("callback is lost (topic={})", topic);
    }
}

// Calls a function using the operating system call to find
// the right target based on the wapm and topic.
// The operating system will respond with either a 'wasm_bus_finish'
// or a 'wasm_bus_error' message.
unsafe fn wasm_bus_call(
    thread: &WasmBusThread,
    parent: Option<CallHandle>,
    handle: CallHandle,
    wapm_ptr: WasmPtr<u8, Array>,
    wapm_len: usize,
    topic_ptr: WasmPtr<u8, Array>,
    topic_len: usize,
    request_ptr: WasmPtr<u8, Array>,
    request_len: usize,
) -> u32 {
    let wapm = wapm_ptr
        .get_utf8_str(thread.memory(), wapm_len as u32)
        .unwrap();
    let topic = topic_ptr
        .get_utf8_str(thread.memory(), topic_len as u32)
        .unwrap();
    if let Some(parent) = parent {
        debug!(
            "wasm-bus::call (parent={}, handle={}, wapm={}, topic={}, request={} bytes)",
            parent.id, handle.id, wapm, topic, request_len
        );
    } else {
        debug!(
            "wasm-bus::call (handle={}, wapm={}, topic={}, request={} bytes)",
            handle.id, wapm, topic, request_len
        );
    }

    let request = thread
        .memory()
        .uint8view_with_byte_offset_and_length(request_ptr.offset(), request_len as u32)
        .to_vec();

    // Grab references to the ABI that will be used
    let data_feeder = match WasmBusCallback::new(thread, handle.into()) {
        Ok(a) => a,
        Err(err) => {
            return err.into();
        }
    };

    // Grab all the client callbacks that have been registered
    let client_callbacks: HashMap<String, WasmBusCallback> = thread
        .inner
        .unwrap()
        .callbacks
        .remove(&handle)
        .map(|a| {
            a.into_iter()
                .map(|(topic, handle)| {
                    (topic, WasmBusCallback::new(thread, handle.into()).unwrap())
                })
                .collect()
        })
        .unwrap_or_default();

    // If its got a parent then we already have an active stream here so we need
    // to feed these results into that stream
    let mut invoke = thread.inner.unwrap().factory.start(
        parent,
        handle.into(),
        wapm.to_string(),
        topic.to_string(),
        request,
        client_callbacks,
    );

    // Invoke the send operation
    let invoke = {
        let thread = thread.clone();
        async move {
            let response = invoke.process().await;
            match response {
                Ok(InvokeResult::Response(response)) => {
                    data_feeder.feed_bytes_or_error(Ok(response));
                }
                Ok(InvokeResult::ResponseThenWork(response, work)) => {
                    data_feeder.feed_bytes_or_error(Ok(response));
                    work.await;
                }
                Err(err) => data_feeder.feed_bytes_or_error(Err(err)),
            }
            thread
                .inner
                .unwrap()
                .factory
                .close(CallHandle::from(handle));
        }
    };

    // We try to invoke the callback synchronously but if it
    // does not complete in time then we add it to the idle
    // processing list which will pick it up again the next time
    // the WASM process yields CPU execution.
    let waker = dummy_waker::dummy_waker();
    let mut cx = Context::from_waker(&waker);
    let mut invoke = Box::pin(invoke);
    if let Poll::Pending = invoke.as_mut().poll(&mut cx) {
        let mut inner = thread.inner.unwrap();
        inner.invocations.insert(handle, invoke);
    }

    // Success
    CallError::Success.into()
}

// Returns a unqiue ID for the thread
unsafe fn wasm_bus_thread_id(thread: &WasmBusThread) -> u32 {
    trace!("wasm-bus::thread_id (id={})", thread.thread_id);
    thread.thread_id
}
