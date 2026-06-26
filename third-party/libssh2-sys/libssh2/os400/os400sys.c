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

/* OS/400 additional support. */

#define LIBSSH2_DISABLE_QADRT_EXT

#include "libssh2_priv.h"

#include <sys/types.h>
#include <sys/socket.h>
#include <sys/un.h>

#include <stdio.h>
#include <stdlib.h>
#include <stddef.h>
#include <stdarg.h>
#include <string.h>
#include <alloca.h>
#include <netdb.h>
#include <qadrt.h>
#include <errno.h>

#include <netinet/in.h>
#include <arpa/inet.h>

#ifdef LIBSSH2_HAVE_ZLIB
# include <zlib.h>
#endif


/**
***     QADRT OS/400 ASCII runtime defines only the most used procedures, but
***             a lot of them are not supported. This module implements
***             ASCII wrappers for those that are used by libssh2, but not
***             defined by QADRT.
**/

#pragma convert(37)                             /* Restore EBCDIC. */


static int
convert_sockaddr(struct sockaddr_storage *dstaddr,
                 const struct sockaddr *srcaddr, int srclen)
{
    const struct sockaddr_un *srcu;
    struct sockaddr_un *dstu;
    unsigned int i;
    unsigned int dstsize;

    /* Convert a socket address into job CCSID, if needed. */

    if(!srcaddr || srclen < offsetof(struct sockaddr, sa_family) +
       sizeof(srcaddr->sa_family) || srclen > sizeof(*dstaddr)) {
        errno = EINVAL;
        return -1;
    }

    memcpy((char *) dstaddr, (char *) srcaddr, srclen);

    switch(srcaddr->sa_family) {

    case AF_UNIX:
        srcu = (const struct sockaddr_un *) srcaddr;
        dstu = (struct sockaddr_un *) dstaddr;
        dstsize = sizeof(*dstaddr) - offsetof(struct sockaddr_un, sun_path);
        srclen -= offsetof(struct sockaddr_un, sun_path);
        i = QadrtConvertA2E(dstu->sun_path, srcu->sun_path,
                            dstsize - 1, srclen);
        dstu->sun_path[i] = '\0';
        i += offsetof(struct sockaddr_un, sun_path);
        srclen = i;
    }

    return srclen;
}


int
_libssh2_os400_connect(int sd, struct sockaddr *destaddr, int addrlen)
{
    int i;
    struct sockaddr_storage laddr;

    i = convert_sockaddr(&laddr, destaddr, addrlen);

    if(i < 0)
        return -1;

    return connect(sd, (struct sockaddr *) &laddr, i);
}


#ifdef LIBSSH2_HAVE_ZLIB
int
_libssh2_os400_inflateInit_(z_streamp strm,
                            const char *version, int stream_size)
{
    char *ebcversion;
    int i;

    if(!version)
        return Z_VERSION_ERROR;
    i = strlen(version);
    ebcversion = alloca(i + 1);
    if(!ebcversion)
        return Z_VERSION_ERROR;
    i = QadrtConvertA2E(ebcversion, version, i, i - 1);
    ebcversion[i] = '\0';
    return inflateInit_(strm, ebcversion, stream_size);
}

int
_libssh2_os400_deflateInit_(z_streamp strm, int level,
                            const char *version, int stream_size)
{
    char *ebcversion;
    int i;

    if(!version)
        return Z_VERSION_ERROR;
    i = strlen(version);
    ebcversion = alloca(i + 1);
    if(!ebcversion)
        return Z_VERSION_ERROR;
    i = QadrtConvertA2E(ebcversion, version, i, i - 1);
    ebcversion[i] = '\0';
    return deflateInit_(strm, level, ebcversion, stream_size);
}

#endif
