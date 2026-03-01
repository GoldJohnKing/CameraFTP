# PASV Port Selection Manual Test

## Test 1: Simple Mode - Default Range Available

**Setup:**
1. Start the app
2. Keep "Advanced Connection" DISABLED
3. Click "Start Server"

**Expected:**
- Server starts successfully
- PASV range: 50000-50100
- Log shows: "Using default PASV range"

---

## Test 2: Simple Mode - Default Range Occupied (Auto-Find)

**Setup:**
1. Occupy ports 50000-50100 with another process:
   ```powershell
   # PowerShell - run this to occupy ports
   $listeners = @()
   50000..50100 | ForEach-Object { 
     try { $listeners += [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Any, $_) 
           $listeners[-1].Start() } catch {}
   }
   Write-Host "Occupied $($listeners.Count) ports"
   Read-Host "Press Enter to release ports"
   ```
2. Start the app with "Advanced Connection" DISABLED
3. Click "Start Server"

**Expected:**
- Server starts successfully
- PASV range: NOT 50000-50100 (auto-found)
- Log shows: "Found available PASV range: XXXXX-XXXXX"

---

## Test 3: Advanced Mode - User Range Available

**Setup:**
1. Enable "Advanced Connection"
2. Set PASV range to 60000-60099
3. Click "Start Server"

**Expected:**
- Server starts successfully
- PASV range: 60000-60099
- Log shows: "Using user-configured PASV range"

---

## Test 4: Advanced Mode - User Range Occupied (Error)

**Setup:**
1. Occupy ports 60000-60009:
   ```powershell
   # PowerShell - occupy all 10 ports
   $listeners = @()
   60000..60009 | ForEach-Object { 
     try { $listeners += [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Any, $_) 
           $listeners[-1].Start() } catch {}
   }
   Write-Host "Occupied $($listeners.Count) ports"
   ```
2. Enable "Advanced Connection"
3. Set PASV range to 60000-60009 (all will be occupied)
4. Click "Start Server"

**Expected:**
- Server FAILS to start
- Error message: "PASV端口范围无可用端口: 60000-60009 (共10个端口均被占用)"
- User sees error in UI

---

## Quick Test (No Port Occupation)

Just test the basic flow:

1. **Simple mode**: Disable "Advanced Connection" → Start → Should work with 50000-50100
2. **Advanced mode**: Enable "Advanced Connection" → Set range 59000-59099 → Start → Should work

---

## How to Check Logs

Look for these log messages:
- `"PASV port range X-Y: N/M available"` - Shows port check result
- `"Using default PASV range"` - Simple mode, default available
- `"Found available PASV range"` - Simple mode, auto-found
- `"Using user-configured PASV range"` - Advanced mode, user range available
- `"NoAvailablePasvPort"` - Advanced mode, all ports occupied
