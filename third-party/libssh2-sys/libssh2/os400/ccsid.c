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

/* Character encoding wrappers. */

#include "libssh2_priv.h"
#include "libssh2_ccsid.h"

#include <qtqiconv.h>
#include <iconv.h>
#include <errno.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>



#define CCSID_UTF8      1208
#define CCSID_UTF16BE   13488
#define STRING_GRANULE  256
#define MAX_CHAR_SIZE   4

#define OFFSET_OF(t, f) ((size_t) ((char *) &((t *) 0)->f - (char *) 0))

#define ALLOC(s, sz)        ((s)? LIBSSH2_ALLOC((s), (sz)): malloc(sz))
#define REALLOC(s, p, sz)   ((s)? LIBSSH2_REALLOC((s), (p), (sz)):      \
                                  realloc((p), (sz)))
#define FREE(s, p)          ((s)? LIBSSH2_FREE((s), (p)): free(p))


struct _libssh2_string_cache {
    libssh2_string_cache *  next;
    char                    string[1];
};


static const QtqCode_T  utf8code = { CCSID_UTF8 };


static ssize_t
terminator_size(unsigned short ccsid)
{
    QtqCode_T outcode;
    iconv_t cd;
    char *inp;
    char *outp;
    size_t ilen;
    size_t olen;
    char buf[MAX_CHAR_SIZE];

    /* Return the null-terminator size for the given CCSID. */

    /* Fast check usual CCSIDs. */
    switch(ccsid) {
    case CCSID_UTF8:
    case 0:                                 /* Job CCSID is SBCS EBCDIC. */
        return 1;
    case CCSID_UTF16BE:
        return 2;
    }

    /* Convert an UTF-8 NUL to the target CCSID: use the converted size as
       result. */
    memset((void *) &outcode, 0, sizeof(outcode));
    outcode.CCSID = ccsid;
    cd = QtqIconvOpen(&outcode, (QtqCode_T *) &utf8code);
    if(cd.return_value == -1)
        return -1;
    inp = "";
    ilen = 1;
    outp = buf;
    olen = sizeof(buf);
    iconv(cd, &inp, &ilen, &outp, &olen);
    iconv_close(cd);
    olen = sizeof(buf - olen);
    return olen ? olen : -1;
}

static char *
convert_ccsid(LIBSSH2_SESSION *session, libssh2_string_cache **cache,
              unsigned short outccsid, unsigned short inccsid,
              const char *instring, ssize_t inlen, size_t *outlen)
{
    char *inp;
    char *outp;
    size_t olen;
    size_t ilen;
    size_t buflen;
    size_t curlen;
    ssize_t termsize;
    int i;
    char *dst;
    libssh2_string_cache *outstring;
    QtqCode_T incode;
    QtqCode_T outcode;
    iconv_t cd;

    if(!instring) {
        if(outlen)
            *outlen = 0;
        return NULL;
    }
    if(outlen)
        *outlen = -1;
    if(!cache)
        return NULL;

    /* Get terminator size. */
    termsize = terminator_size(outccsid);
    if(termsize < 0)
        return NULL;

    /* Prepare conversion parameters. */
    memset((void *) &incode, 0, sizeof(incode));
    memset((void *) &outcode, 0, sizeof(outcode));
    incode.CCSID = inccsid;
    outcode.CCSID = outccsid;
    curlen = OFFSET_OF(libssh2_string_cache, string);
    inp = (char *) instring;
    ilen = inlen;
    buflen = inlen + curlen;
    if(inlen < 0) {
        incode.length_option = 1;
        buflen = STRING_GRANULE;
        ilen = 0;
    }

    /* Allocate output string buffer and open conversion descriptor. */
    dst = ALLOC(session, buflen + termsize);
    if(!dst)
        return NULL;
    cd = QtqIconvOpen(&outcode, &incode);
    if(cd.return_value == -1) {
        FREE(session, dst);
        return NULL;
    }

    /* Convert string. */
    for(;;) {
        outp = dst + curlen;
        olen = buflen - curlen;
        i = iconv(cd, &inp, &ilen, &outp, &olen);
        if(inlen < 0 && olen == buflen - curlen) {
            /* Special case: converted 0-length (sub)strings do not store the
               terminator. */
            if(termsize) {
                memset(outp, 0, termsize);
                olen -= termsize;
            }
        }
        curlen = buflen - olen;
        if(i >= 0 || errno != E2BIG)
            break;
        /* Must expand buffer. */
        buflen += STRING_GRANULE;
        outp = REALLOC(session, dst, buflen + termsize);
        if(!outp)
            break;
        dst = outp;
    }

    iconv_close(cd);

    /* Check for error. */
    if(i < 0 || !outp) {
        FREE(session, dst);
        return NULL;
    }

    /* Process terminator. */
    if(inlen < 0)
        curlen -= termsize;
    else if(termsize)
        memset(dst + curlen, 0, termsize);

    /* Shorten buffer if possible. */
    if(curlen < buflen)
        dst = REALLOC(session, dst, curlen + termsize);

    /* Link to cache. */
    outstring = (libssh2_string_cache *) dst;
    outstring->next = *cache;
    *cache = outstring;

    /* Return length if required. */
    if(outlen)
        *outlen = curlen - OFFSET_OF(libssh2_string_cache, string);

    return outstring->string;
}

LIBSSH2_API char *
libssh2_from_ccsid(LIBSSH2_SESSION *session, libssh2_string_cache **cache,
                   unsigned short ccsid, const char *string, ssize_t inlen,
                   size_t *outlen)
{
    return convert_ccsid(session, cache,
                         CCSID_UTF8, ccsid, string, inlen, outlen);
}

LIBSSH2_API char *
libssh2_to_ccsid(LIBSSH2_SESSION *session, libssh2_string_cache **cache,
                 unsigned short ccsid, const char *string, ssize_t inlen,
                 size_t *outlen)
{
    return convert_ccsid(session, cache,
                         ccsid, CCSID_UTF8, string, inlen, outlen);
}

LIBSSH2_API void
libssh2_release_string_cache(LIBSSH2_SESSION *session,
                             libssh2_string_cache **cache)
{
    libssh2_string_cache *p;

    if(cache)
        while((p = *cache)) {
            *cache = p->next;
            FREE(session, (char *) p);
        }
}

/* vim: set expandtab ts=4 sw=4: */
