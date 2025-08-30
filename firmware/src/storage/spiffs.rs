// src/storage/spiffs.rs

// The `use` statement is now very simple.
use esp_idf_svc::sys::{esp_spiffs_format, esp_spiffs_info, esp_vfs_spiffs_conf_t, esp_vfs_spiffs_register, esp_vfs_spiffs_unregister, EspError, ESP_OK};
use log::{error, info};
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};

/// Represents a SPIFFS filesystem manager that ensures proper mounting and unmounting.
/// This struct handles the lifecycle of the SPIFFS filesystem, including mounting, formatting,
/// and unmounting operations. It implements RAII (Resource Acquisition Is Initialization)
/// to ensure the filesystem is properly unmounted when the struct is dropped.
pub struct SpiffsManager {
    partition_label: CString,
}

impl SpiffsManager {
    /// Initializes and mounts the SPIFFS partition using direct sys calls.
    /// This will automatically format the partition if it cannot be mounted.
    pub fn new(partition_label: &str) -> Result<Self, EspError> {
        info!("Initializing SPIFFS on partition '{}'...", partition_label);

        // C-style strings are required for the FFI (C API) calls.
        let base_path = CString::new("/spiffs").unwrap();
        let c_partition_label = CString::new(partition_label).unwrap();

        // This is the C configuration struct, as you correctly defined.
        let conf = esp_vfs_spiffs_conf_t {
            base_path: base_path.as_ptr(),
            partition_label: c_partition_label.as_ptr(),
            max_files: 5,
            format_if_mount_failed: true,
        };

        // Call the underlying C function within an `unsafe` block.
        let ret = unsafe { esp_vfs_spiffs_register(&conf) };

        // Check the return code. If it's not OK, we have a fatal error.
        if ret != ESP_OK {
            error!("Failed to mount or format SPIFFS. Error code: {}", ret);
            return Err(EspError::from(ret).unwrap());
        }

        // Get and log partition info
        let mut total_bytes: usize = 0;
        let mut used_bytes: usize = 0;
        unsafe {
            // We pass the C-string label to get info for our specific partition.
            esp_spiffs_info(c_partition_label.as_ptr(), &mut total_bytes, &mut used_bytes);
        }
        info!(
            "SPIFFS partition '{}' mounted: total bytes: {}, used bytes: {}",
            partition_label, total_bytes, used_bytes
        );
        
        Ok(Self {
            partition_label: c_partition_label,
        })
    }   
    
    
    /// Formats the SPIFFS partition. THIS WILL ERASE ALL DATA.
    /// The filesystem does NOT need to be mounted for this to work.
    pub fn format(partition_label: &str) -> Result<(), EspError> {
        info!("Formatting SPIFFS partition '{}'...", partition_label);
    
        // Convert the label to a C-style string.
        let c_partition_label = CString::new(partition_label).unwrap();

        // Call the dedicated format function from the C API.
        let ret = unsafe { esp_spiffs_format(c_partition_label.as_ptr()) };

        if ret != ESP_OK {
            error!("Failed to format SPIFFS. Error code: {}", ret);
            Err(EspError::from(ret).unwrap())
        } else {
            info!("Partition '{}' formatted successfully.", partition_label);
            Ok(())
        }
    }


    pub fn verify_filesystem(&self) -> Result<(), std::io::Error> {
    let test_path = "/spiffs/test.txt";
    let test_content = "test";
    
    // First try to remove any existing test file (ignore errors)
    let _ = std::fs::remove_file(test_path);
    
    // Write test file
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)  // Use create_new to ensure fresh file
        .open(test_path)?;
    file.write_all(test_content.as_bytes())?;
    file.sync_all()?;  // Force flush to storage
    drop(file);
    
    // Read back
    let mut file = File::open(test_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;
    
    if contents != test_content {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "File content mismatch",
        ));
    }
    
    // Clean up
    std::fs::remove_file(test_path)?;
    
    Ok(())
}

    // --- The file I/O functions are UNCHANGED ---
    // They rely on the standard `std::fs` library, which works correctly
    // as long as the VFS was registered by `Spiffs::new`.

    pub fn write_file(&self, path: &str, data: &str) -> Result<(), std::io::Error> {
        //let full_path = format!("/spiffs/{}", path);
        info!("Writing {} bytes to '{}'", data.len(), path);
        
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
            
        file.write_all(data.as_bytes())?;
        Ok(())
    }

    pub fn read_file(&self, path: &str) -> Result<String, std::io::Error> {
        //let full_path = format!("/spiffs/{}", path);
        info!("Reading from '{}'", path);
        
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        Ok(contents)
    }

    pub fn file_exists(&self, path: &str) -> bool {
        //let full_path = format!("/spiffs/{}", path);
        std::path::Path::new(path).exists()
    }
    pub fn delete_file(&self, path: &str) -> Result<(), std::io::Error> {
        //let full_path = format!("/spiffs/{}", path);
        info!("Deleting file '{}'", path);

        std::fs::remove_file(path)?;
        Ok(())
    }
}



/// The Drop trait implementation ensures that when the SpiffsManager goes out
/// of scope, the filesystem is cleanly unmounted and its resources are freed.
impl Drop for SpiffsManager {
    fn drop(&mut self) {
        info!("Unmounting SPIFFS partition '{}'...", self.partition_label.to_str().unwrap());
        let ret = unsafe { esp_vfs_spiffs_unregister(self.partition_label.as_ptr()) };
        if ret != ESP_OK {
            error!("Failed to unmount SPIFFS. Error code: {}", ret);
        } else {
            info!("SPIFFS unmounted successfully.");
        }
    }
}