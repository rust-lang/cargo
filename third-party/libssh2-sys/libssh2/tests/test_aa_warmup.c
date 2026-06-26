/* Copyright (C) Viktor Szakats
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

/* Warm-up test. Always return success.
   Workaround for CI/docker/etc flakiness on the first run. */

#include "runner.h"

int test(LIBSSH2_SESSION *session)
{
    size_t len = 0;
    int type = 0;
    const char *hostkey = libssh2_session_hostkey(session, &len, &type);

    (void)hostkey;

    fprintf(stdout,
            "libssh2_session_hostkey returned len, type: %ld, %d\n",
            (long)len, type);

    return 0;  /* always return success */
}
