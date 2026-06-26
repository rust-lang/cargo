/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "runner.h"

int test(LIBSSH2_SESSION *session)
{
#if LIBSSH2_RSA_SHA1 && LIBSSH2_AES_GCM && \
    (defined(LIBSSH2_OPENSSL) || defined(LIBSSH2_WOLFSSL))
    /* set in Dockerfile */
    return test_auth_pubkey(session, 0,
                            "libssh2",
                            "libssh2",
                            "key_rsa_aes256gcm.pub",
                            "key_rsa_aes256gcm");
#else
    (void)session;
    return 0;
#endif
}
