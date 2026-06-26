/* Copyright (C) The libssh2 project and its contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "runner.h"

int test(LIBSSH2_SESSION *session)
{
    return test_auth_password(session, 0,
                              "libssh2", /* set in Dockerfile */
                              "my test password"); /* set in Dockerfile */
}
