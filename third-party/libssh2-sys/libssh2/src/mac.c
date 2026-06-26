/* Copyright (C) Sara Golemon <sarag@libssh2.org>
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

#include "libssh2_priv.h"
#include "mac.h"

#if defined(LIBSSH2DEBUG) && defined(LIBSSH2_MAC_NONE_INSECURE)
/* mac_none_MAC
 *
 * Minimalist MAC: No MAC. DO NOT USE.
 *
 * The SSH2 Transport allows implementations to forego a message
 * authentication code.  While this is less of a security risk than using
 * a "none" cipher, it is still not recommended as disabling MAC hashes
 * removes a layer of security.
 *
 * Enabling this option will allow for "none" as a negotiable method,
 * however it still requires that the method be advertised by the remote
 * end and that no more-preferable methods are available.
 *
 */
static int
mac_none_MAC(LIBSSH2_SESSION * session, unsigned char *buf,
             uint32_t seqno, const unsigned char *packet,
             size_t packet_len, const unsigned char *addtl,
             size_t addtl_len, void **abstract)
{
    return 0;
}




static LIBSSH2_MAC_METHOD mac_method_none = {
    "none",
    0,
    0,
    NULL,
    mac_none_MAC,
    NULL,
    0
};
#endif /* defined(LIBSSH2DEBUG) && defined(LIBSSH2_MAC_NONE_INSECURE) */

/* mac_method_common_init
 * Initialize simple mac methods
 */
static int
mac_method_common_init(LIBSSH2_SESSION * session, unsigned char *key,
                       int *free_key, void **abstract)
{
    *abstract = key;
    *free_key = 0;
    (void)session;

    return 0;
}



/* mac_method_common_dtor
 * Cleanup simple mac methods
 */
static int
mac_method_common_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    if(*abstract) {
        LIBSSH2_FREE(session, *abstract);
    }
    *abstract = NULL;

    return 0;
}



#if LIBSSH2_HMAC_SHA512
/* mac_method_hmac_sha512_hash
 * Calculate hash using full sha512 value
 */
static int
mac_method_hmac_sha2_512_hash(LIBSSH2_SESSION * session,
                              unsigned char *buf, uint32_t seqno,
                              const unsigned char *packet,
                              size_t packet_len,
                              const unsigned char *addtl,
                              size_t addtl_len, void **abstract)
{
    libssh2_hmac_ctx ctx;
    unsigned char seqno_buf[4];
    int res;
    (void)session;

    _libssh2_htonu32(seqno_buf, seqno);

    if(!_libssh2_hmac_ctx_init(&ctx))
        return 1;
    res = _libssh2_hmac_sha512_init(&ctx, *abstract, 64) &&
          _libssh2_hmac_update(&ctx, seqno_buf, 4) &&
          _libssh2_hmac_update(&ctx, packet, packet_len);
    if(res && addtl && addtl_len)
        res = _libssh2_hmac_update(&ctx, addtl, addtl_len);
    if(res)
        res = _libssh2_hmac_final(&ctx, buf);
    _libssh2_hmac_cleanup(&ctx);

    return !res;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_sha2_512 = {
    "hmac-sha2-512",
    64,
    64,
    mac_method_common_init,
    mac_method_hmac_sha2_512_hash,
    mac_method_common_dtor,
    0
};

static const LIBSSH2_MAC_METHOD mac_method_hmac_sha2_512_etm = {
    "hmac-sha2-512-etm@openssh.com",
    64,
    64,
    mac_method_common_init,
    mac_method_hmac_sha2_512_hash,
    mac_method_common_dtor,
    1
};

#endif



#if LIBSSH2_HMAC_SHA256
/* mac_method_hmac_sha256_hash
 * Calculate hash using full sha256 value
 */
static int
mac_method_hmac_sha2_256_hash(LIBSSH2_SESSION * session,
                              unsigned char *buf, uint32_t seqno,
                              const unsigned char *packet,
                              size_t packet_len,
                              const unsigned char *addtl,
                              size_t addtl_len, void **abstract)
{
    libssh2_hmac_ctx ctx;
    unsigned char seqno_buf[4];
    int res;
    (void)session;

    _libssh2_htonu32(seqno_buf, seqno);

    if(!_libssh2_hmac_ctx_init(&ctx))
        return 1;
    res = _libssh2_hmac_sha256_init(&ctx, *abstract, 32) &&
          _libssh2_hmac_update(&ctx, seqno_buf, 4) &&
          _libssh2_hmac_update(&ctx, packet, packet_len);
    if(res && addtl && addtl_len)
        res = _libssh2_hmac_update(&ctx, addtl, addtl_len);
    if(res)
        res = _libssh2_hmac_final(&ctx, buf);
    _libssh2_hmac_cleanup(&ctx);

    return !res;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_sha2_256 = {
    "hmac-sha2-256",
    32,
    32,
    mac_method_common_init,
    mac_method_hmac_sha2_256_hash,
    mac_method_common_dtor,
    0
};

static const LIBSSH2_MAC_METHOD mac_method_hmac_sha2_256_etm = {
    "hmac-sha2-256-etm@openssh.com",
    32,
    32,
    mac_method_common_init,
    mac_method_hmac_sha2_256_hash,
    mac_method_common_dtor,
    1
};

#endif




/* mac_method_hmac_sha1_hash
 * Calculate hash using full sha1 value
 */
static int
mac_method_hmac_sha1_hash(LIBSSH2_SESSION * session,
                          unsigned char *buf, uint32_t seqno,
                          const unsigned char *packet,
                          size_t packet_len,
                          const unsigned char *addtl,
                          size_t addtl_len, void **abstract)
{
    libssh2_hmac_ctx ctx;
    unsigned char seqno_buf[4];
    int res;
    (void)session;

    _libssh2_htonu32(seqno_buf, seqno);

    if(!_libssh2_hmac_ctx_init(&ctx))
        return 1;
    res = _libssh2_hmac_sha1_init(&ctx, *abstract, 20) &&
          _libssh2_hmac_update(&ctx, seqno_buf, 4) &&
          _libssh2_hmac_update(&ctx, packet, packet_len);
    if(res && addtl && addtl_len)
        res = _libssh2_hmac_update(&ctx, addtl, addtl_len);
    if(res)
        res = _libssh2_hmac_final(&ctx, buf);
    _libssh2_hmac_cleanup(&ctx);

    return !res;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_sha1 = {
    "hmac-sha1",
    20,
    20,
    mac_method_common_init,
    mac_method_hmac_sha1_hash,
    mac_method_common_dtor,
    0
};

static const LIBSSH2_MAC_METHOD mac_method_hmac_sha1_etm = {
    "hmac-sha1-etm@openssh.com",
    20,
    20,
    mac_method_common_init,
    mac_method_hmac_sha1_hash,
    mac_method_common_dtor,
    1
};

/* mac_method_hmac_sha1_96_hash
 * Calculate hash using first 96 bits of sha1 value
 */
static int
mac_method_hmac_sha1_96_hash(LIBSSH2_SESSION * session,
                             unsigned char *buf, uint32_t seqno,
                             const unsigned char *packet,
                             size_t packet_len,
                             const unsigned char *addtl,
                             size_t addtl_len, void **abstract)
{
    unsigned char temp[SHA_DIGEST_LENGTH];

    if(mac_method_hmac_sha1_hash(session, temp, seqno, packet, packet_len,
                                 addtl, addtl_len, abstract))
        return 1;

    memcpy(buf, (char *) temp, 96 / 8);
    return 0;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_sha1_96 = {
    "hmac-sha1-96",
    12,
    20,
    mac_method_common_init,
    mac_method_hmac_sha1_96_hash,
    mac_method_common_dtor,
    0
};

#if LIBSSH2_MD5
/* mac_method_hmac_md5_hash
 * Calculate hash using full md5 value
 */
static int
mac_method_hmac_md5_hash(LIBSSH2_SESSION * session, unsigned char *buf,
                         uint32_t seqno,
                         const unsigned char *packet,
                         size_t packet_len,
                         const unsigned char *addtl,
                         size_t addtl_len, void **abstract)
{
    libssh2_hmac_ctx ctx;
    unsigned char seqno_buf[4];
    int res;
    (void)session;

    _libssh2_htonu32(seqno_buf, seqno);

    if(!_libssh2_hmac_ctx_init(&ctx))
        return 1;
    res = _libssh2_hmac_md5_init(&ctx, *abstract, 16) &&
          _libssh2_hmac_update(&ctx, seqno_buf, 4) &&
          _libssh2_hmac_update(&ctx, packet, packet_len);
    if(res && addtl && addtl_len)
        res = _libssh2_hmac_update(&ctx, addtl, addtl_len);
    if(res)
        res = _libssh2_hmac_final(&ctx, buf);
    _libssh2_hmac_cleanup(&ctx);

    return !res;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_md5 = {
    "hmac-md5",
    16,
    16,
    mac_method_common_init,
    mac_method_hmac_md5_hash,
    mac_method_common_dtor,
    0
};

/* mac_method_hmac_md5_96_hash
 * Calculate hash using first 96 bits of md5 value
 */
static int
mac_method_hmac_md5_96_hash(LIBSSH2_SESSION * session,
                            unsigned char *buf, uint32_t seqno,
                            const unsigned char *packet,
                            size_t packet_len,
                            const unsigned char *addtl,
                            size_t addtl_len, void **abstract)
{
    unsigned char temp[MD5_DIGEST_LENGTH];

    if(mac_method_hmac_md5_hash(session, temp, seqno, packet, packet_len,
                                addtl, addtl_len, abstract))
        return 1;

    memcpy(buf, (char *) temp, 96 / 8);
    return 0;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_md5_96 = {
    "hmac-md5-96",
    12,
    16,
    mac_method_common_init,
    mac_method_hmac_md5_96_hash,
    mac_method_common_dtor,
    0
};
#endif /* LIBSSH2_MD5 */

#if LIBSSH2_HMAC_RIPEMD
/* mac_method_hmac_ripemd160_hash
 * Calculate hash using ripemd160 value
 */
static int
mac_method_hmac_ripemd160_hash(LIBSSH2_SESSION * session,
                               unsigned char *buf, uint32_t seqno,
                               const unsigned char *packet,
                               size_t packet_len,
                               const unsigned char *addtl,
                               size_t addtl_len,
                               void **abstract)
{
    libssh2_hmac_ctx ctx;
    unsigned char seqno_buf[4];
    int res;
    (void)session;

    _libssh2_htonu32(seqno_buf, seqno);

    if(!_libssh2_hmac_ctx_init(&ctx))
        return 1;
    res = _libssh2_hmac_ripemd160_init(&ctx, *abstract, 20) &&
          _libssh2_hmac_update(&ctx, seqno_buf, 4) &&
          _libssh2_hmac_update(&ctx, packet, packet_len);
    if(res && addtl && addtl_len)
        res = _libssh2_hmac_update(&ctx, addtl, addtl_len);
    if(res)
        res = _libssh2_hmac_final(&ctx, buf);
    _libssh2_hmac_cleanup(&ctx);

    return !res;
}



static const LIBSSH2_MAC_METHOD mac_method_hmac_ripemd160 = {
    "hmac-ripemd160",
    20,
    20,
    mac_method_common_init,
    mac_method_hmac_ripemd160_hash,
    mac_method_common_dtor,
    0
};

static const LIBSSH2_MAC_METHOD mac_method_hmac_ripemd160_openssh_com = {
    "hmac-ripemd160@openssh.com",
    20,
    20,
    mac_method_common_init,
    mac_method_hmac_ripemd160_hash,
    mac_method_common_dtor,
    0
};
#endif /* LIBSSH2_HMAC_RIPEMD */

static const LIBSSH2_MAC_METHOD *mac_methods[] = {
#if LIBSSH2_HMAC_SHA256
    &mac_method_hmac_sha2_256,
    &mac_method_hmac_sha2_256_etm,
#endif
#if LIBSSH2_HMAC_SHA512
    &mac_method_hmac_sha2_512,
    &mac_method_hmac_sha2_512_etm,
#endif
    &mac_method_hmac_sha1,
    &mac_method_hmac_sha1_etm,
    &mac_method_hmac_sha1_96,
#if LIBSSH2_MD5
    &mac_method_hmac_md5,
    &mac_method_hmac_md5_96,
#endif
#if LIBSSH2_HMAC_RIPEMD
    &mac_method_hmac_ripemd160,
    &mac_method_hmac_ripemd160_openssh_com,
#endif /* LIBSSH2_HMAC_RIPEMD */
#if defined(LIBSSH2DEBUG) && defined(LIBSSH2_MAC_NONE_INSECURE)
    &mac_method_none,
#endif
    NULL
};

const LIBSSH2_MAC_METHOD **
_libssh2_mac_methods(void)
{
    return mac_methods;
}

#if LIBSSH2_AES_GCM
static int
mac_method_none_init(LIBSSH2_SESSION * session, unsigned char *key,
                     int *free_key, void **abstract)
{
    (void)session;
    (void)key;
    (void)free_key;
    (void)abstract;
    return 0;
}

static int
mac_method_hmac_none_hash(LIBSSH2_SESSION * session,
                          unsigned char *buf, uint32_t seqno,
                          const unsigned char *packet,
                          size_t packet_len,
                          const unsigned char *addtl,
                          size_t addtl_len, void **abstract)
{
    (void)session;
    (void)buf;
    (void)seqno;
    (void)packet;
    (void)packet_len;
    (void)addtl;
    (void)addtl_len;
    (void)abstract;
    return 0;
}

static int
mac_method_none_dtor(LIBSSH2_SESSION * session, void **abstract)
{
    (void)session;
    (void)abstract;
    return 0;
}

/* Stub for aes256-gcm@openssh.com crypto type, which has an integrated
   HMAC method. This must not be added to mac_methods[] since it cannot be
   negotiated separately. */
static const LIBSSH2_MAC_METHOD mac_method_hmac_aesgcm = {
    "INTEGRATED-AES-GCM",  /* made up name for display only */
    16,
    16,
    mac_method_none_init,
    mac_method_hmac_none_hash,
    mac_method_none_dtor,
    0
};
#endif /* LIBSSH2_AES_GCM */

/* See if the negotiated crypto method has its own authentication scheme that
 * obviates the need for a separate negotiated hmac method */
const LIBSSH2_MAC_METHOD *
_libssh2_mac_override(const LIBSSH2_CRYPT_METHOD *crypt)
{
#if LIBSSH2_AES_GCM
    if(!strcmp(crypt->name, "aes256-gcm@openssh.com") ||
       !strcmp(crypt->name, "aes128-gcm@openssh.com"))
        return &mac_method_hmac_aesgcm;
#else
    (void) crypt;
#endif /* LIBSSH2_AES_GCM */
    return NULL;
}
