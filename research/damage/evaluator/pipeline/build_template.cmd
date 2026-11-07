@echo off
call "C:\Program Files\Microsoft Visual Studio\18\Community\VC\Auxiliary\Build\vcvars64.bat" >nul
"C:\Program Files\NVIDIA GPU Computing Toolkit\CUDA\v13.3\bin\nvcc.exe" -O2 -std=c++17 --fmad=false --gpu-architecture=sm_89 -Wno-deprecated-gpu-targets --diag-suppress=177 path_kernel.cu -o path_kernel.exe
