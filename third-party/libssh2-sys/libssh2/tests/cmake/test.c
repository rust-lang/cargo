/* Copyright (C) Viktor Szakats
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "libssh2.h"
#include <stdio.h>

int main(void)
{
    printf("libssh2_version(0): |%s|\n", libssh2_version(0));
    return 0;
}
