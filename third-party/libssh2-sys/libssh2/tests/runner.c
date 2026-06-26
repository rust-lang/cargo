/* Copyright (C) Alexander Lamaison
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms,
 * with or without modification, are permitted provided
 * that the following conditions are met:
 *
 *   Redistributions of source code must retain the above
 *   copyright notice, this list of conditions and the
 *   following disclaimer.
 *
 *   Redistributions in binary form must reproduce the above
 *   copyright notice, this list of conditions and the following
 *   disclaimer in the documentation and/or other materials
 *   provided with the distribution.
 *
 *   Neither the name of the copyright holder nor the names
 *   of any other contributors may be used to endorse or
 *   promote products derived from this software without
 *   specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 * CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
 * USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
 * OF SUCH DAMAGE.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include "runner.h"

int main(void)
{
    int exit_code;
    int retries = 0, retry = 0;

#ifdef LIBSSH2_WINCNG
    /* FIXME: Retry tests with WinCNG due to flakiness in hostkey
       verification: https://github.com/libssh2/libssh2/issues/804 */
    retries += 2;
#endif

    do {
        int skipped, rc;
        LIBSSH2_SESSION *session = start_session_fixture(&skipped, &rc);
        if(session) {
            exit_code = (test(session) == 0) ? 0 : 1;
        }
        else if(skipped) {
            fprintf(stderr, "Test skipped.\n");
            exit_code = 0;
        }
        else {
            exit_code = 1;
        }
        stop_session_fixture();
        if(exit_code == 0 ||
#ifdef LIBSSH2_WINCNG
           rc != LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE ||
#endif
           ++retry > retries) {
            break;
        }
        fprintf(stderr, "Test failed (%d). Retrying... %d / %d\n",
                        rc, retry, retries);
    } while(1);

    return exit_code;
}
