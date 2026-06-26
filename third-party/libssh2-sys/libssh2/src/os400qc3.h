#ifndef LIBSSH2_OS400QC3_H
#define LIBSSH2_OS400QC3_H
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

#define LIBSSH2_CRYPTO_ENGINE libssh2_os400qc3

#include <stdlib.h>
#include <string.h>

#include <qc3cci.h>


/* Redefine character/string literals as always EBCDIC. */
#undef Qc3_Alg_Token
#define Qc3_Alg_Token       "\xC1\xD3\xC7\xC4\xF0\xF1\xF0\xF0"  /* ALGD0100 */
#undef Qc3_Alg_Block_Cipher
#define Qc3_Alg_Block_Cipher "\xC1\xD3\xC7\xC4\xF0\xF2\xF0\xF0" /* ALGD0200 */
#undef Qc3_Alg_Block_CipherAuth
#define Qc3_Alg_Block_CipherAuth                                            \
                            "\xC1\xD3\xC7\xC4\xF0\xF2\xF1\xF0"  /* ALGD0210 */
#undef Qc3_Alg_Stream_Cipher
#define Qc3_Alg_Stream_Cipher                                               \
                            "\xC1\xD3\xC7\xC4\xF0\xF3\xF0\xF0"  /* ALGD0300 */
#undef Qc3_Alg_Public_Key
#define Qc3_Alg_Public_Key  "\xC1\xD3\xC7\xC4\xF0\xF4\xF0\xF0"  /* ALGD0400 */
#undef Qc3_Alg_Hash
#define Qc3_Alg_Hash        "\xC1\xD3\xC7\xC4\xF0\xF5\xF0\xF0"  /* ALGD0500 */
#undef Qc3_Data
#define Qc3_Data            "\xC4\xC1\xE3\xC1\xF0\xF1\xF0\xF0"  /* DATA0100 */
#undef Qc3_Array
#define Qc3_Array           "\xC4\xC1\xE3\xC1\xF0\xF2\xF0\xF0"  /* DATA0200 */
#undef Qc3_Key_Token
#define Qc3_Key_Token       "\xD2\xC5\xE8\xC4\xF0\xF1\xF0\xF0"  /* KEYD0100 */
#undef Qc3_Key_Parms
#define Qc3_Key_Parms       "\xD2\xC5\xE8\xC4\xF0\xF2\xF0\xF0"  /* KEYD0200 */
#undef Qc3_Key_KSLabel
#define Qc3_Key_KSLabel     "\xD2\xC5\xE8\xC4\xF0\xF4\xF0\xF0"  /* KEYD0400 */
#undef Qc3_Key_PKCS5
#define Qc3_Key_PKCS5       "\xD2\xC5\xE8\xC4\xF0\xF5\xF0\xF0"  /* KEYD0500 */
#undef Qc3_Key_PEMCert
#define Qc3_Key_PEMCert     "\xD2\xC5\xE8\xC4\xF0\xF6\xF0\xF0"  /* KEYD0600 */
#undef Qc3_Key_CSLabel
#define Qc3_Key_CSLabel     "\xD2\xC5\xE8\xC4\xF0\xF7\xF0\xF0"  /* KEYD0700 */
#undef Qc3_Key_CSDN
#define Qc3_Key_CSDN        "\xD2\xC5\xE8\xC4\xF0\xF8\xF0\xF0"  /* KEYD0800 */
#undef Qc3_Key_AppID
#define Qc3_Key_AppID       "\xD2\xC5\xE8\xC4\xF0\xF9\xF0\xF0"  /* KEYD0900 */

#undef Qc3_ECB
#define Qc3_ECB             '\xF0'      /* '0' */
#undef Qc3_CBC
#define Qc3_CBC             '\xF1'      /* '1' */
#undef Qc3_OFB
#define Qc3_OFB             '\xF2'      /* '2' */
#undef Qc3_CFB1Bit
#define Qc3_CFB1Bit         '\xF3'      /* '3' */
#undef Qc3_CFB8Bit
#define Qc3_CFB8Bit         '\xF4'      /* '4' */
#undef Qc3_CFB64Bit
#define Qc3_CFB64Bit        '\xF5'      /* '5' */
#undef Qc3_CUSP
#define Qc3_CUSP            '\xF6'      /* '6' */
#undef Qc3_CTR
#define Qc3_CTR             '\xF7'      /* '7' */
#undef Qc3_CCM
#define Qc3_CCM             '\xF8'      /* '8' */
#undef Qc3_No_Pad
#define Qc3_No_Pad          '\xF0'      /* '0' */
#undef Qc3_Pad_Char
#define Qc3_Pad_Char        '\xF1'      /* '1' */
#undef Qc3_Pad_Counter
#define Qc3_Pad_Counter     '\xF2'      /* '2' */
#undef Qc3_PKCS1_00
#define Qc3_PKCS1_00        '\xF0'      /* '0' */
#undef Qc3_PKCS1_01
#define Qc3_PKCS1_01        '\xF1'      /* '1' */
#undef Qc3_PKCS1_02
#define Qc3_PKCS1_02        '\xF2'      /* '2' */
#undef Qc3_ISO9796
#define Qc3_ISO9796         '\xF3'      /* '3' */
#undef Qc3_Zero_Pad
#define Qc3_Zero_Pad        '\xF4'      /* '4' */
#undef Qc3_ANSI_X931
#define Qc3_ANSI_X931       '\xF5'      /* '5' */
#undef Qc3_OAEP
#define Qc3_OAEP            '\xF6'      /* '6' */
#undef Qc3_Bin_String
#define Qc3_Bin_String      '\xF0'      /* '0' */
#undef Qc3_BER_String
#define Qc3_BER_String      '\xF1'      /* '1' */
#undef Qc3_MK_Struct
#define Qc3_MK_Struct       '\xF3'      /* '3' */
#undef Qc3_KSLabel_Struct
#define Qc3_KSLabel_Struct  '\xF4'      /* '4' */
#undef Qc3_PKCS5_Struct
#define Qc3_PKCS5_Struct    '\xF5'      /* '5' */
#undef Qc3_PEMCert_String
#define Qc3_PEMCert_String  '\xF6'      /* '6' */
#undef Qc3_CSLabel_String
#define Qc3_CSLabel_String  '\xF7'      /* '7' */
#undef Qc3_CSDN_String
#define Qc3_CSDN_String     '\xF8'      /* '8' */
#undef Qc3_Clear
#define Qc3_Clear           '\xF0'      /* '0' */
#undef Qc3_Encrypted
#define Qc3_Encrypted       '\xF1'      /* '1' */
#undef Qc3_MK_Encrypted
#define Qc3_MK_Encrypted    '\xF2'      /* '2' */
#undef Qc3_Any_CSP
#define Qc3_Any_CSP         '\xF0'      /* '0' */
#undef Qc3_Sfw_CSP
#define Qc3_Sfw_CSP         '\xF1'      /* '1' */
#undef Qc3_Hdw_CSP
#define Qc3_Hdw_CSP         '\xF2'      /* '2' */
#undef Qc3_Continue
#define Qc3_Continue        '\xF0'      /* '0' */
#undef Qc3_Final
#define Qc3_Final           '\xF1'      /* '1' */
#undef Qc3_MK_New
#define Qc3_MK_New          '\xF0'      /* '0' */
#undef Qc3_MK_Current
#define Qc3_MK_Current      '\xF1'      /* '1' */
#undef Qc3_MK_Old
#define Qc3_MK_Old          '\xF2'      /* '2' */
#undef Qc3_MK_Pending
#define Qc3_MK_Pending      '\xF3'      /* '3' */

/* Define which features are supported. */
#ifdef OPENSSL_NO_MD5
# define LIBSSH2_MD5            0
#else
# define LIBSSH2_MD5            1
#endif

#define LIBSSH2_HMAC_RIPEMD     0
#define LIBSSH2_HMAC_SHA256     1
#define LIBSSH2_HMAC_SHA512     1

#define LIBSSH2_AES_CBC         1
#define LIBSSH2_AES_CTR         1
#define LIBSSH2_AES_GCM         0
#define LIBSSH2_BLOWFISH        0
#define LIBSSH2_RC4             1
#define LIBSSH2_CAST            0
#define LIBSSH2_3DES            1

#define LIBSSH2_RSA             1
#define LIBSSH2_RSA_SHA1        1
#define LIBSSH2_RSA_SHA2        1
#define LIBSSH2_DSA             0
#define LIBSSH2_ECDSA           0
#define LIBSSH2_ED25519         0

#include "crypto_config.h"

#define SHA_DIGEST_LENGTH       20
#define SHA256_DIGEST_LENGTH    32
#define SHA384_DIGEST_LENGTH    48
#define SHA512_DIGEST_LENGTH    64

#define EC_MAX_POINT_LEN ((528 * 2 / 8) + 1)

#if LIBSSH2_ECDSA
#else
#define _libssh2_ec_key void
#endif

/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: global handles structures.
 *
 *******************************************************************/

/* HMAC & private key algorithms support structure. */
typedef struct _libssh2_os400qc3_crypto_ctx _libssh2_os400qc3_crypto_ctx;
struct _libssh2_os400qc3_crypto_ctx {
    Qc3_Format_ALGD0100_T           hash;           /* Hash algorithm. */
    Qc3_Format_KEYD0100_T           key;            /* Key. */
    _libssh2_os400qc3_crypto_ctx *  kek;            /* Key encryption. */
};

typedef struct {        /* Big number. */
    unsigned char *         bignum;         /* Number bits, little-endian. */
    unsigned int            length;         /* Length of bignum (# bytes). */
}       _libssh2_bn;

typedef struct {        /* Algorithm description. */
    char *                  fmt;            /* Format of Qc3 structure. */
    int                     algo;           /* Algorithm identifier. */
    unsigned char           size;           /* Block length. */
    unsigned char           mode;           /* Block mode. */
    int                     keylen;         /* Key length. */
}       _libssh2_os400qc3_cipher_t;

typedef struct {        /* Diffie-Hellman context. */
    char                    token[8];       /* Context token. */
}       _libssh2_os400qc3_dh_ctx;

/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: Define global types/codes.
 *
 *******************************************************************/

#define libssh2_crypto_init()
#define libssh2_crypto_exit()

#define libssh2_sha1_ctx        Qc3_Format_ALGD0100_T
#define libssh2_sha256_ctx      Qc3_Format_ALGD0100_T
#define libssh2_sha384_ctx      Qc3_Format_ALGD0100_T
#define libssh2_sha512_ctx      Qc3_Format_ALGD0100_T
#define libssh2_hmac_ctx        _libssh2_os400qc3_crypto_ctx
#define _libssh2_cipher_ctx     _libssh2_os400qc3_crypto_ctx

#define libssh2_sha1_init(x)    _libssh2_os400qc3_hash_init(x, Qc3_SHA1)
#define libssh2_sha1_update(ctx, data, len)                                 \
                               _libssh2_os400qc3_hash_update(&(ctx), data, len)
#define libssh2_sha1_final(ctx, out)                                        \
                                _libssh2_os400qc3_hash_final(&(ctx), out)
#define libssh2_sha256_init(x)  _libssh2_os400qc3_hash_init(x, Qc3_SHA256)
#define libssh2_sha256_update(ctx, data, len)                               \
                               _libssh2_os400qc3_hash_update(&(ctx), data, len)
#define libssh2_sha256_final(ctx, out)                                      \
                                _libssh2_os400qc3_hash_final(&(ctx), out)
#define libssh2_sha256(message, len, out)                                   \
                                _libssh2_os400qc3_hash(message, len, out,   \
                                                       Qc3_SHA256)
#define libssh2_sha384_init(x)  _libssh2_os400qc3_hash_init(x, Qc3_SHA384)
#define libssh2_sha384_update(ctx, data, len)                               \
                               _libssh2_os400qc3_hash_update(&(ctx), data, len)
#define libssh2_sha384_final(ctx, out)                                      \
                                _libssh2_os400qc3_hash_final(&(ctx), out)
#define libssh2_sha384(message, len, out)                                   \
                                _libssh2_os400qc3_hash(message, len, out,   \
                                                       Qc3_SHA384)
#define libssh2_sha512_init(x)  _libssh2_os400qc3_hash_init(x, Qc3_SHA512)
#define libssh2_sha512_update(ctx, data, len)                               \
                               _libssh2_os400qc3_hash_update(&(ctx), data, len)
#define libssh2_sha512_final(ctx, out)                                      \
                                _libssh2_os400qc3_hash_final(&(ctx), out)
#define libssh2_sha512(message, len, out)                                   \
                                _libssh2_os400qc3_hash(message, len, out,   \
                                                       Qc3_SHA512)

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#define MD5_DIGEST_LENGTH       16
#define libssh2_md5_ctx         Qc3_Format_ALGD0100_T
#define libssh2_md5_init(x)     _libssh2_os400qc3_hash_init(x, Qc3_MD5)
#define libssh2_md5_update(ctx, data, len)                                  \
                               _libssh2_os400qc3_hash_update(&(ctx), data, len)
#define libssh2_md5_final(ctx, out)                                         \
                                _libssh2_os400qc3_hash_final(&(ctx), out)
#endif

#define _libssh2_bn_ctx         int                 /* Not used. */

#define _libssh2_bn_ctx_new()           0
#define _libssh2_bn_ctx_free(bnctx)     ((void) 0)

#define _libssh2_bn_init_from_bin() _libssh2_bn_init()
#define _libssh2_bn_bytes(bn)   ((bn)->length)

#define _libssh2_cipher_type(name)  _libssh2_os400qc3_cipher_t name
#define _libssh2_cipher_aes128 {Qc3_Alg_Block_Cipher, Qc3_AES, 16,          \
                                Qc3_CBC, 16}
#define _libssh2_cipher_aes192 {Qc3_Alg_Block_Cipher, Qc3_AES, 16,          \
                                Qc3_CBC, 24}
#define _libssh2_cipher_aes256 {Qc3_Alg_Block_Cipher, Qc3_AES, 16,          \
                                Qc3_CBC, 32}
#define _libssh2_cipher_aes128ctr {Qc3_Alg_Block_Cipher, Qc3_AES, 16,       \
                                   Qc3_CTR, 16}
#define _libssh2_cipher_aes192ctr {Qc3_Alg_Block_Cipher, Qc3_AES, 16,       \
                                   Qc3_CTR, 24}
#define _libssh2_cipher_aes256ctr {Qc3_Alg_Block_Cipher, Qc3_AES, 16,       \
                                   Qc3_CTR, 32}
#define _libssh2_cipher_3des {Qc3_Alg_Block_Cipher, Qc3_TDES, 8,            \
                              Qc3_CBC, 24}
/* Nonsense values for chacha20-poly1305 */
#define _libssh2_cipher_chacha20 {Qc3_Alg_Stream_Cipher, Qc3_RC4, 8, 0, 16}
#define _libssh2_cipher_arcfour {Qc3_Alg_Stream_Cipher, Qc3_RC4, 8, 0, 16}

#define _libssh2_cipher_dtor(ctx) _libssh2_os400qc3_crypto_dtor(ctx)

#define libssh2_rsa_ctx         _libssh2_os400qc3_crypto_ctx
#define _libssh2_rsa_free(ctx)  (_libssh2_os400qc3_crypto_dtor(ctx),        \
                                 free((char *) ctx))
#define libssh2_prepare_iovec(vec, len) memset((char *) (vec), 0,           \
                                               (len) * sizeof(struct iovec))
#define _libssh2_rsa_sha1_signv(session, sig, siglen, count, vector, ctx)   \
            _libssh2_os400qc3_rsa_signv(session, Qc3_SHA1, sig, siglen,     \
                                             count, vector, ctx)
#define _libssh2_rsa_sha2_256_signv(session, sig, siglen, cnt, vector, ctx) \
            _libssh2_os400qc3_rsa_signv(session, Qc3_SHA256, sig, siglen,   \
                                             cnt, vector, ctx)
#define _libssh2_rsa_sha2_512_signv(session, sig, siglen, cnt, vector, ctx) \
            _libssh2_os400qc3_rsa_signv(session, Qc3_SHA512, sig, siglen,   \
                                             cnt, vector, ctx)

/* Default generate and safe prime sizes for diffie-hellman-group-exchange-sha1
   Qc3 is limited to a maximum 2048-bit modulus/key size. */
#define LIBSSH2_DH_GEX_MINGROUP     1024
#define LIBSSH2_DH_GEX_OPTGROUP     1536
#define LIBSSH2_DH_GEX_MAXGROUP     2048

#define LIBSSH2_DH_MAX_MODULUS_BITS 2048

#define _libssh2_dh_ctx         _libssh2_os400qc3_dh_ctx
#define libssh2_dh_init(dhctx)  _libssh2_os400qc3_dh_init(dhctx)
#define libssh2_dh_key_pair(dhctx, public, g, p, group_order, bnctx)        \
            _libssh2_os400qc3_dh_key_pair(dhctx, public, g, p, group_order)
#define libssh2_dh_secret(dhctx, secret, f, p, bnctx)                       \
            _libssh2_os400qc3_dh_secret(dhctx, secret, f, p)
#define libssh2_dh_dtor(dhctx)  _libssh2_os400qc3_dh_dtor(dhctx)


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: Support procedure prototypes.
 *
 *******************************************************************/

extern _libssh2_bn *    _libssh2_bn_init(void);
extern void     _libssh2_bn_free(_libssh2_bn *bn);
extern unsigned long    _libssh2_bn_bits(_libssh2_bn *bn);
extern int      _libssh2_bn_from_bin(_libssh2_bn *bn, size_t len,
                                     const unsigned char *v);
extern int      _libssh2_bn_set_word(_libssh2_bn *bn, unsigned long val);
extern int      _libssh2_bn_to_bin(_libssh2_bn *bn, unsigned char *val);
extern int      _libssh2_random(unsigned char *buf, size_t len);
extern void     _libssh2_os400qc3_crypto_dtor(_libssh2_os400qc3_crypto_ctx *x);
extern int      _libssh2_os400qc3_hash_init(Qc3_Format_ALGD0100_T *x,
                                            unsigned int algo);
extern int      _libssh2_os400qc3_hash_update(Qc3_Format_ALGD0100_T *ctx,
                                              const unsigned char *data,
                                              int len);
extern int      _libssh2_os400qc3_hash_final(Qc3_Format_ALGD0100_T *ctx,
                                             unsigned char *out);
extern int      _libssh2_os400qc3_hash(const unsigned char *message,
                                       unsigned long len, unsigned char *out,
                                       unsigned int algo);
extern int      _libssh2_os400qc3_rsa_signv(LIBSSH2_SESSION *session, int algo,
                                            unsigned char **signature,
                                            size_t *signature_len,
                                            int veccount,
                                            const struct iovec vector[],
                                            libssh2_rsa_ctx *ctx);
extern void     _libssh2_os400qc3_dh_init(_libssh2_dh_ctx *dhctx);
extern int      _libssh2_os400qc3_dh_key_pair(_libssh2_dh_ctx *dhctx,
                                              _libssh2_bn *public,
                                              _libssh2_bn *g,
                                              _libssh2_bn *p, int group_order);
extern int      _libssh2_os400qc3_dh_secret(_libssh2_dh_ctx *dhctx,
                                            _libssh2_bn *secret,
                                            _libssh2_bn *f, _libssh2_bn *p);
extern void     _libssh2_os400qc3_dh_dtor(_libssh2_dh_ctx *dhctx);

#endif /* LIBSSH2_OS400QC3_H */

/* vim: set expandtab ts=4 sw=4: */
