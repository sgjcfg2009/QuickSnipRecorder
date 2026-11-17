# QuickSnip Recorder (Rust)

Công cụ quay màn hình, chọn vùng tùy chỉnh và quay riêng từng ứng dụng (kèm **WASAPI Process Loopback** chỉ thu âm thanh từ đúng App đó) viết bằng **Rust + Direct2D + DirectWrite + WASAPI + FFmpeg**.

## Tính năng
- **Quay Toàn màn hình / Chọn vùng / Quay riêng App** (tự động tách âm thanh riêng của App).
- **60+ FPS mượt mà**: Tự động nhận diện phần cứng GPU (NVENC, QSV, AMF, MediaFoundation HW) và DXGI Desktop Duplication (ddagrab).
- **1 File EXE duy nhất**: uild.bat tự động tải Toolchain Rust portable + fmpeg.exe và đóng gói tất cả vào một file QuickSnipRecorder.exe độc lập.
- **Phím tắt toàn cục**: F9 / Ctrl+Shift+R (Bắt đầu / Dừng), F7 (Tạm dừng / Tiếp tục), F8 (Hủy bỏ).