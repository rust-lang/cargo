#ifndef LIBSSH2_LIBGCRYPT_H
#define LIBSSH2_LIBGCRYPT_H
/*
 * Copyright (C) Simon Josefsson
 * Copyright (C) The Written Word, Inc.
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

#define LIBSSH2_CRYPTO_ENGINE libssh2_gcrypt

#include <gcrypt.h>

#define LIBSSH2_MD5 1

#define LIBSSH2_HMAC_RIPEMD 1
#define LIBSSH2_HMAC_SHA256 1
#define LIBSSH2_HMAC_SHA512 1

#define LIBSSH2_AES_CBC 1
#define LIBSSH2_AES_CTR 1
#define LIBSSH2_AES_GCM 0
#define LIBSSH2_BLOWFISH 1
#define LIBSSH2_RC4 1
#define LIBSSH2_CAST 1
#define LIBSSH2_3DES 1

#define LIBSSH2_RSA 1
#define LIBSSH2_RSA_SHA1 1
#define LIBSSH2_RSA_SHA2 0
#define LIBSSH2_DSA 1
#define LIBSSH2_ECDSA 0
#define LIBSSH2_ED25519 0

#include "crypto_config.h"

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define MD5_DIGEST_LENGTH 16
#endif
#define SHA_DIGEST_LENGTH 20
#define SHA256_DIGEST_LENGTH 32
#define SHA384_DIGEST_LENGTH 48
#define SHA512_DIGEST_LENGTH 64

#define EC_MAX_POINT_LEN ((528 * 2 / 8) + 1)

#define _libssh2_random(buf, len) \
    (gcry_randomize((buf), (len), GCRY_STRONG_RANDOM), 0)

#define libssh2_prepare_iovec(vec, len)  /* Empty. */

#define libssh2_sha1_ctx gcry_md_hd_t
/* returns 0 in case of failure */
#define libssh2_sha1_init(ctx) \
    (GPG_ERR_NO_ERROR == gcry_md_open(ctx, GCRY_MD_SHA1, 0))
#define libssh2_sha1_update(ctx, data, len) \
    (gcry_md_write(ctx, (unsigned char *) data, len), 1)
#define libssh2_sha1_final(ctx, out) \
    (memcpy(out, gcry_md_read(ctx, 0), SHA_DIGEST_LENGTH), \
    gcry_md_close(ctx), 1)
#define libssh2_sha1(message, len, out) \
    (gcry_md_hash_buffer(GCRY_MD_SHA1, out, message, len), 0)

#define libssh2_sha256_ctx gcry_md_hd_t
#define libssh2_sha256_init(ctx) \
    (GPG_ERR_NO_ERROR == gcry_md_open(ctx, GCRY_MD_SHA256, 0))
#define libssh2_sha256_update(ctx, data, len) \
    (gcry_md_write(ctx, (unsigned char *) data, len), 1)
#define libssh2_sha256_final(ctx, out) \
    (memcpy(out, gcry_md_read(ctx, 0), SHA256_DIGEST_LENGTH), \
    gcry_md_close(ctx), 1)
#define libssh2_sha256(message, len, out) \
    (gcry_md_hash_buffer(GCRY_MD_SHA256, out, message, len), 0)

#define libssh2_sha384_ctx gcry_md_hd_t
#define libssh2_sha384_init(ctx) \
    (GPG_ERR_NO_ERROR == gcry_md_open(ctx, GCRY_MD_SHA384, 0))
#define libssh2_sha384_update(ctx, data, len) \
    (gcry_md_write(ctx, (unsigned char *) data, len), 1)
#define libssh2_sha384_final(ctx, out) \
    (memcpy(out, gcry_md_read(ctx, 0), SHA384_DIGEST_LENGTH), \
    gcry_md_close(ctx), 1)
#define libssh2_sha384(message, len, out) \
    (gcry_md_hash_buffer(GCRY_MD_SHA384, out, message, len), 0)

#define libssh2_sha512_ctx gcry_md_hd_t
#define libssh2_sha512_init(ctx) \
    (GPG_ERR_NO_ERROR == gcry_md_open(ctx, GCRY_MD_SHA512, 0))
#define libssh2_sha512_update(ctx, data, len) \
    (gcry_md_write(ctx, (unsigned char *) data, len), 1)
#define libssh2_sha512_final(ctx, out) \
    (memcpy(out, gcry_md_read(ctx, 0), SHA512_DIGEST_LENGTH), \
    gcry_md_close(ctx), 1)
#define libssh2_sha512(message, len, out) \
    (gcry_md_hash_buffer(GCRY_MD_SHA512, out, message, len), 0)

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define libssh2_md5_ctx gcry_md_hd_t
#define libssh2_md5_init(ctx) \
    (GPG_ERR_NO_ERROR == gcry_md_open(ctx, GCRY_MD_MD5, 0))
#define libssh2_md5_update(ctx, data, len) \
    (gcry_md_write(ctx, (unsigned char *) data, len), 1)
#define libssh2_md5_final(ctx, out) \
    (memcpy(out, gcry_md_read(ctx, 0), MD5_DIGEST_LENGTH), \
    gcry_md_close(ctx), 1)
#endif

#define libssh2_hmac_ctx gcry_md_hd_t

#define libssh2_crypto_init() gcry_control(GCRYCTL_DISABLE_SECMEM)
#define libssh2_crypto_exit()

#define libssh2_rsa_ctx struct gcry_sexp

#define _libssh2_rsa_free(rsactx)  gcry_sexp_release(rsactx)

#define libssh2_dsa_ctx struct gcry_sexp

#define _libssh2_dsa_free(dsactx)  gcry_sexp_release(dsactx)

#if LIBSSH2_ECDSA
#else
#define _libssh2_ec_key void
#endif

#define _libssh2_cipher_type(name) int name
#define _libssh2_cipher_ctx gcry_cipher_hd_t

#define _libssh2_gcry_ciphermode(c,m) ((c << 8) | m)
#define _libssh2_gcry_cipher(c) (c >> 8)
#define _libssh2_gcry_mode(m) (m & 0xFF)

#define _libssh2_cipher_aes256ctr \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES256, GCRY_CIPHER_MODE_CTR)
#define _libssh2_cipher_aes192ctr \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES192, GCRY_CIPHER_MODE_CTR)
#define _libssh2_cipher_aes128ctr \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES128, GCRY_CIPHER_MODE_CTR)
#define _libssh2_cipher_aes256 \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES256, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_aes192 \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES192, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_aes128 \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_AES128, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_blowfish \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_BLOWFISH, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_arcfour \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_ARCFOUR, GCRY_CIPHER_MODE_STREAM)
#define _libssh2_cipher_cast5 \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_CAST5, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_3des \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_3DES, GCRY_CIPHER_MODE_CBC)
#define _libssh2_cipher_chacha20 \
    _libssh2_gcry_ciphermode(GCRY_CIPHER_CHACHA20, GCRY_CIPHER_MODE_STREAM)


#define _libssh2_cipher_dtor(ctx) gcry_cipher_close(*(ctx))

#define _libssh2_bn struct gcry_mpi
#define _libssh2_bn_ctx int
#define _libssh2_bn_ctx_new() 0
#define _libssh2_bn_ctx_free(bnctx) ((void)0)
#define _libssh2_bn_init() gcry_mpi_new(0)
#define _libssh2_bn_init_from_bin() NULL  /* because gcry_mpi_scan() creates a
                                             new bignum */
#define _libssh2_bn_set_word(bn, val) gcry_mpi_set_ui(bn, val)
#define _libssh2_bn_from_bin(bn, len, val) \
    gcry_mpi_scan(&((bn)), GCRYMPI_FMT_USG, val, len, NULL)
#define _libssh2_bn_to_bin(bn, val) \
    gcry_mpi_print(GCRYMPI_FMT_USG, val, _libssh2_bn_bytes(bn), NULL, bn)
#define _libssh2_bn_bytes(bn) \
    (gcry_mpi_get_nbits(bn) / 8 + \
         ((gcry_mpi_get_nbits(bn) % 8 == 0) ? 0 : 1))
#define _libssh2_bn_bits(bn) gcry_mpi_get_nbits(bn)
#define _libssh2_bn_free(bn) gcry_mpi_release(bn)

/* Default generate and safe prime sizes for
   diffie-hellman-group-exchange-sha1 */
#define LIBSSH2_DH_GEX_MINGROUP     2048
#define LIBSSH2_DH_GEX_OPTGROUP     4096
#define LIBSSH2_DH_GEX_MAXGROUP     8192

#define LIBSSH2_DH_MAX_MODULUS_BITS 16384

#define _libssh2_dh_ctx struct gcry_mpi *
#define libssh2_dh_init(dhctx) _libssh2_dh_init(dhctx)
#define libssh2_dh_key_pair(dhctx, public, g, p, group_order, bnctx) \
    _libssh2_dh_key_pair(dhctx, public, g, p, group_order)
#define libssh2_dh_secret(dhctx, secret, f, p, bnctx) \
    _libssh2_dh_secret(dhctx, secret, f, p)
#define libssh2_dh_dtor(dhctx) _libssh2_dh_dtor(dhctx)
extern void _libssh2_init_aes_ctr(void);
extern void _libssh2_dh_init(_libssh2_dh_ctx *dhctx);
extern int _libssh2_dh_key_pair(_libssh2_dh_ctx *dhctx, _libssh2_bn *public,
                                _libssh2_bn *g, _libssh2_bn *p,
                                int group_order);
extern int _libssh2_dh_secret(_libssh2_dh_ctx *dhctx, _libssh2_bn *secret,
                              _libssh2_bn *f, _libssh2_bn *p);
extern void _libssh2_dh_dtor(_libssh2_dh_ctx *dhctx);

#endif /* LIBSSH2_LIBGCRYPT_H */
