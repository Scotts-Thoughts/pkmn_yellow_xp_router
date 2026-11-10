// C ABI of the xpr-mobile crate (rust/crates/xpr-mobile/src/ffi.rs).
// Every call is a JSON command: xpr_call("method", "{params}") returns a
// JSON envelope {"ok": <value>} or {"err": "message"}. Strings returned by
// the library must be released with xpr_free_string. One session per
// process; all calls must come from one thread at a time (the Swift side
// serialises them on its own queue).
#ifndef XPR_MOBILE_H
#define XPR_MOBILE_H

#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Start the session. `config_dir` holds config.json and the log;
/// `data_dir` the saved routes and custom gens. Returns a JSON envelope.
char *xpr_init(const char *config_dir, const char *data_dir);

/// Run one command; see Session::dispatch for the method names.
char *xpr_call(const char *method, const char *params_json);

/// Render a PNG for an image request (map pictures, ...). Returns the byte
/// count and stores a buffer to free with xpr_free_bytes in *out, or 0.
size_t xpr_png(const char *method, const char *params_json, uint8_t **out);

void xpr_free_string(char *s);
void xpr_free_bytes(uint8_t *p, size_t len);

#ifdef __cplusplus
}
#endif

#endif
