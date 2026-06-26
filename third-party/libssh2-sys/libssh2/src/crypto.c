/* Copyright (C) Viktor Szakats
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#define LIBSSH2_CRYPTO_C
#include "libssh2_priv.h"

#if defined(LIBSSH2_OPENSSL) || defined(LIBSSH2_WOLFSSL)
#include "openssl.c"
#elif defined(LIBSSH2_LIBGCRYPT)
#include "libgcrypt.c"
#elif defined(LIBSSH2_MBEDTLS)
#include "mbedtls.c"
#elif defined(LIBSSH2_OS400QC3)
#include "os400qc3.c"
#elif defined(LIBSSH2_WINCNG)
#include "wincng.c"
#endif
