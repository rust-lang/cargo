/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "runner.h"

int test(LIBSSH2_SESSION *session)
{
#if LIBSSH2_ED25519
    /* set in Dockerfile */
    return test_auth_pubkey(session, 0,
                            "libssh2",
                            "libssh2",
                            "key_ed25519_encrypted.pub",
                            "key_ed25519_encrypted");
#else
    (void)session;
    return 0;
#endif
}
