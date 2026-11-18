use std::{
    error::Error,
    fs::File,
    io::{BufWriter, Error as IoError},
    path::Path,
};

use windows::{
    Win32::{
        Foundation::{HWND, RECT},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS,
            DeleteDC, DeleteObject, GetDIBits, GetWindowDC, HDC, ReleaseDC, SRCCOPY, SelectObject,
        },
        Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow},
        UI::WindowsAndMessaging::GetWindowRect,
    },
    core::Error as WindowsError,
};

pub fn capture_window(hwnd: HWND, path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }

    let mut rect = RECT::default();
    unsafe { GetWindowRect(hwnd, &mut rect) }
        .map_err(|error| windows_error("GetWindowRect", error))?;
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return Err(IoError::other("window has invalid capture dimensions").into());
    }

    let window_dc = unsafe { GetWindowDC(Some(hwnd)) };
    if window_dc.is_invalid() {
        return Err(os_error("GetWindowDC").into());
    }
    let result = capture_pixels(hwnd, window_dc, width, height);
    unsafe { ReleaseDC(Some(hwnd), window_dc) };
    let mut rgba = result?;
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }

    let file = File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width as u32, height as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(&rgba)?;
    Ok(())
}

fn os_error(operation: &str) -> IoError {
    let error = IoError::last_os_error();
    IoError::new(error.kind(), format!("{operation}: {error}"))
}

fn windows_error(operation: &str, error: WindowsError) -> IoError {
    IoError::other(format!("{operation}: {error}"))
}

fn capture_pixels(
    hwnd: HWND,
    window_dc: HDC,
    width: i32,
    height: i32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let memory_dc = unsafe { CreateCompatibleDC(Some(window_dc)) };
    if memory_dc.is_invalid() {
        return Err(os_error("CreateCompatibleDC").into());
    }
    let bitmap = unsafe { CreateCompatibleBitmap(window_dc, width, height) };
    if bitmap.is_invalid() {
        let _ = unsafe { DeleteDC(memory_dc) };
        return Err(os_error("CreateCompatibleBitmap").into());
    }
    let previous = unsafe { SelectObject(memory_dc, bitmap.into()) };

    let mut info = BITMAPINFO::default();
    info.bmiHeader.biSize = size_of_val(&info.bmiHeader) as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = -height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB.0;
    let length = width as usize * height as usize * 4;
    info.bmiHeader.biSizeImage = length as u32;
    let mut rgba = vec![0; length];
    let result = (|| {
        let captured = unsafe {
            BitBlt(
                memory_dc,
                0,
                0,
                width,
                height,
                Some(window_dc),
                0,
                0,
                SRCCOPY,
            )
        }
        .is_ok()
            || unsafe { PrintWindow(hwnd, memory_dc, PRINT_WINDOW_FLAGS(1)) } != false;
        if !captured {
            return Err(os_error("BitBlt and PrintWindow fallback"));
        }
        let rows = unsafe {
            GetDIBits(
                memory_dc,
                bitmap,
                0,
                height as u32,
                Some(rgba.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            )
        };
        if rows != height {
            return Err(os_error("GetDIBits"));
        }
        Ok(())
    })();

    unsafe {
        SelectObject(memory_dc, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory_dc);
    }
    result?;
    Ok(rgba)
}
