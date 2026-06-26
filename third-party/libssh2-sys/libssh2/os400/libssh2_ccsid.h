/*
 * Copyright (C) Patrick Monnerat <patrick@monnerat.net>
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

/* CCSID conversion support. */

#ifndef LIBSSH2_CCSID_H_
#define LIBSSH2_CCSID_H_

#include "libssh2.h"

typedef struct _libssh2_string_cache    libssh2_string_cache;


LIBSSH2_API char *
libssh2_from_ccsid(LIBSSH2_SESSION *session, libssh2_string_cache **cache,
                   unsigned short ccsid, const char *string, ssize_t inlen,
                   size_t *outlen);
LIBSSH2_API char *
libssh2_to_ccsid(LIBSSH2_SESSION *session, libssh2_string_cache **cache,
                 unsigned short ccsid, const char *string, ssize_t inlen,
                 size_t *outlen);
LIBSSH2_API void
libssh2_release_string_cache(LIBSSH2_SESSION *session,
                             libssh2_string_cache **cache);

#endif

/* vim: set expandtab ts=4 sw=4: */
