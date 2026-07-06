use std::time::Duration;

use futures::{future, future::Either};

use crate::{ExitStatus, LifecycleManager};

#[tokio::test]
async fn test_basic() -> Result<(), Box<dyn std::error::Error>> {
    let lifecycle_manager = LifecycleManager::<()>::new();

    let _lifecycle_manager = lifecycle_manager.spawn("future 1", |signal| async {
        println!("future 1 is working");

        let sleep = tokio::time::sleep(Duration::from_secs(3));
        tokio::pin!(sleep);

        match future::select(signal, sleep).await {
            Either::Left(_) => println!("future 1 got shutdown signal"),
            Either::Right(_) => println!("future 1 is completed"),
        }

        ExitStatus::Success
    });

    let _result = lifecycle_manager.serve().await?.ok();
    Ok(())
}

#[tokio::test]
async fn test_basic_with_sigterm() -> Result<(), Box<dyn std::error::Error>> {
    let pid = std::process::id();
    println!("PID: {pid}");

    // Spawn another thread to kill `pid`.
    let _thread = std::thread::spawn(move || {
        // Sleep for 1 second.
        std::thread::sleep(Duration::from_secs(1));

        // SAFETY: We want to send UNIX signal to the process itself.
        #[expect(unsafe_code, reason = "Test helper needs to send SIGTERM via libc")]
        #[cfg(unix)]
        unsafe {
            // Send SIGTERM to terminate `pid`
            let result = libc::kill(pid.cast_signed(), libc::SIGTERM);
            assert_eq!(result, 0);
        }

        #[cfg(windows)]
        {
            // Windows equivalent logic using Command to call 'taskkill'.
            std::process::Command::new("taskkill")
                .arg("/F")
                .arg("/PID")
                .arg(pid.to_string())
                .spawn()
                .expect("failed to kill self");
        }

        println!("Process {pid} has been terminated");
    });

    let lifecycle_manager = LifecycleManager::<()>::new();

    let _lifecycle_manager = lifecycle_manager.spawn("future 1", |signal| async {
        println!("future 1 is working");

        let sleep = tokio::time::sleep(Duration::from_secs(30));
        tokio::pin!(sleep);

        match future::select(signal, sleep).await {
            Either::Left(_) => println!("future 1 got shutdown signal"),
            Either::Right(_) => println!("future 1 is completed"),
        }

        ExitStatus::Success
    });

    let _result = lifecycle_manager.serve().await?.ok();
    Ok(())
}
