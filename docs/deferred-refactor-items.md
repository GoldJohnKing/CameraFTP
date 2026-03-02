# Deferred Refactor Items

This document records codebase cleanup and refactor items that were analyzed but **not implemented**, along with the reasons for deferral.

---

## LOW Priority Items

### LOW-1: `registerEvents` Function

**Location:** `src/utils/events.ts:25-48`

**Analysis:**
- `registerEvents` is a private helper function (not exported)
- It has exactly one usage: called internally by `createEventManager.registerAll()` at line 81
- `createEventManager` is the exported public API, providing:
  - Batch registration via `registerAll()`
  - Incremental registration via `on()`
  - Cleanup guard with `isCleanedUp` flag

**Decision:** Keep as internal helper

**Reason:**
- Serves a valid DRY purpose within the module
- `createEventManager` provides a strictly better public API
- No external use case that would benefit from direct access
- Keeping it private maintains a clean public API surface

---

### LOW-2: `NetworkInterface` Fields

**Location:** `src-tauri/src/network.rs:5-10`

**Original Concern:**
```rust
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub is_wifi: bool,      // Thought to be unused
    pub is_ethernet: bool,  // Thought to be unused
}
```

**Analysis:**
Fields ARE being used for IP selection priority logic:
```rust
// Line 136 - WiFi priority
if let Some(iface) = interfaces.iter().find(|i| i.is_wifi) { ... }

// Line 142 - Ethernet secondary priority
if let Some(iface) = interfaces.iter().find(|i| i.is_ethernet) { ... }
```

**Decision:** No changes needed

**Reason:**
- Fields are actively used for network interface prioritization
- Selection priority: WiFi > Ethernet > Other

---

### LOW-3: Empty Catch Blocks

**Locations:** 16+ locations across the codebase

**Examples:**
```typescript
// App.tsx:53,92
try {
    await invoke('hide_main_window');
} catch {
    // Silently ignore window hide errors
}
```

**Decision:** Skip (per user request)

**Reason:**
- These empty catch blocks are intentional - non-critical operations
- Adding logging would create noise during normal operation
- Would require per-case evaluation of whether logging is appropriate

---

## MEDIUM Priority Items

### MED-2: `checkPermissions` Refactoring

**Location:** `src/stores/permissionStore.ts:111-134`

**Issue:**
The `checkPermissions` function has a return pattern incompatible with `executeAsync`:
- On error, returns `get().permissions` (current state) instead of `undefined`
- Has "partial success" handling (`perms` is `null` treated as error but doesn't throw)

**Current Pattern:**
```typescript
checkPermissions: async () => {
  try {
    const perms = await permissionCheckInternal();
    if (perms) {
      set({ permissions: perms, allGranted, isLoading: false });
      return perms;
    } else {
      set({ isLoading: false, error: 'Failed to check permissions' });
      return get().permissions;  // Returns current state, not undefined
    }
  } catch (err) {
    set({ isLoading: false, error: errorMsg });
    return get().permissions;  // Returns current state, not undefined
  }
}
```

**Decision:** Keep manual implementation

**Reason:**
- `executeAsync` returns `undefined` on error, but `checkPermissions` must return current state
- Refactoring would require modifying `executeAsync` API or creating a wrapper
- Low benefit - current code works correctly

---

### MED-4: `FtpAuthConfig` vs `AuthConfig` Merge

**Locations:**
- `src-tauri/src/config.rs` - `AuthConfig`
- `src-tauri/src/ftp/types.rs` - `FtpAuthConfig`

**Analysis:**
| Field           | `AuthConfig` | `FtpAuthConfig` |
|-----------------|--------------|-----------------|
| anonymous       | ✅           | ✅              |
| username        | ✅           | ✅              |
| password_hash   | ✅           | ✅              |
| password_salt   | ✅           | ❌              |

**Decision:** Keep separate

**Reason:**
- Different responsibilities:
  - `AuthConfig` - Full config storage (includes salt for password management)
  - `FtpAuthConfig` - Runtime FTP authentication (simplified, no salt needed)
- `From<&AuthConfig> for FtpAuthConfig` already handles conversion cleanly
- Intentional separation of concerns (storage vs runtime)

---

### MED-6: Android Path Functions Consolidation

**Location:** `src-tauri/src/platform/android.rs:53-78`

**Functions:**
```rust
// is_path_writable (called 2 times)
fn is_path_writable(path: &std::path::Path) -> bool { ... }

// can_write_to_dcim (called 2 times)
fn can_write_to_dcim() -> bool { ... }
```

**Analysis:**
Both functions are called from multiple locations:
- `is_path_writable`: `can_write_to_dcim()` (L71), `validate_path_writable()` (L105)
- `can_write_to_dcim`: `get_storage_info()` (L26), `check_all_files_permission()` (L49)

**Decision:** Keep as-is (only added clarifying comments)

**Reason:**
- Both functions have multiple callers - inline expansion not appropriate
- Functions have different abstraction levels:
  - `is_path_writable` - Low-level primitive, tests write capability
  - `can_write_to_dcim` - Business logic, checks specific DCIM path
- Current design follows single responsibility principle
- Adding comment to clarify `is_path_writable` behavior (doesn't check existence)

---

## Summary

| Item          | Category    | Reason                           |
|---------------|-------------|----------------------------------|
| LOW-1 registerEvents | Keep | Internal helper, good API design |
| LOW-2 NetworkInterface fields | Keep | In use for IP priority selection |
| LOW-3 Empty catch blocks | Skip | Intentional silent handling |
| MED-2 checkPermissions | Keep | Incompatible return pattern |
| MED-4 AuthConfig merge | Keep | Intentional separation of concerns |
| MED-6 Android path functions | Keep | Different abstraction levels |

---

*Document created during codebase cleanup session (2025-03-02)*
