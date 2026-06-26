/*
 * Copyright (C) Marc Hoersken <info@marc-hoersken.de>
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

#ifdef LIBSSH2_CRYPTO_C /* Compile this via crypto.c */

/* required for cross-compilation against the w64 mingw-runtime package */
#if defined(_WIN32_WINNT) && (_WIN32_WINNT < 0x0600)
#undef _WIN32_WINNT
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif

#if !defined(LIBSSH2_WINCNG_DISABLE_WINCRYPT) && !defined(HAVE_LIBCRYPT32)
#define HAVE_LIBCRYPT32
#endif

/* specify the required libraries for dependencies using MSVC */
#ifdef _MSC_VER
#pragma comment(lib, "bcrypt.lib")
#ifdef HAVE_LIBCRYPT32
#pragma comment(lib, "crypt32.lib")
#endif
#endif

#include <windows.h>
#include <bcrypt.h>
#include <math.h>

#include <stdlib.h>

#ifdef HAVE_LIBCRYPT32
#include <wincrypt.h>  /* for CryptDecodeObjectEx() */
#endif

#define PEM_RSA_HEADER "-----BEGIN RSA PRIVATE KEY-----"
#define PEM_RSA_FOOTER "-----END RSA PRIVATE KEY-----"
#define PEM_DSA_HEADER "-----BEGIN DSA PRIVATE KEY-----"
#define PEM_DSA_FOOTER "-----END DSA PRIVATE KEY-----"
#define PEM_ECDSA_HEADER "-----BEGIN OPENSSH PRIVATE KEY-----"
#define PEM_ECDSA_FOOTER "-----END OPENSSH PRIVATE KEY-----"

#define OPENSSL_PRIVATEKEY_AUTH_MAGIC "openssh-key-v1"

/* Define these manually to avoid including <ntstatus.h> and thus
   clashing with <windows.h> symbols. */
#ifndef STATUS_NOT_SUPPORTED
#define STATUS_NOT_SUPPORTED ((NTSTATUS)0xC00000BB)
#endif

#ifndef STATUS_INVALID_SIGNATURE
#define STATUS_INVALID_SIGNATURE ((NTSTATUS)0xC000A000)
#endif

/*******************************************************************/
/*
 * Windows CNG backend: Missing definitions (for MinGW[-w64])
 */
#ifndef BCRYPT_SUCCESS
#define BCRYPT_SUCCESS(Status) (((NTSTATUS)(Status)) >= 0)
#endif

#ifndef BCRYPT_RNG_ALGORITHM
#define BCRYPT_RNG_ALGORITHM L"RNG"
#endif

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
#ifndef BCRYPT_MD5_ALGORITHM
#define BCRYPT_MD5_ALGORITHM L"MD5"
#endif
#endif

#ifndef BCRYPT_SHA1_ALGORITHM
#define BCRYPT_SHA1_ALGORITHM L"SHA1"
#endif

#ifndef BCRYPT_SHA256_ALGORITHM
#define BCRYPT_SHA256_ALGORITHM L"SHA256"
#endif

#ifndef BCRYPT_SHA384_ALGORITHM
#define BCRYPT_SHA384_ALGORITHM L"SHA384"
#endif

#ifndef BCRYPT_SHA512_ALGORITHM
#define BCRYPT_SHA512_ALGORITHM L"SHA512"
#endif

#ifndef BCRYPT_RSA_ALGORITHM
#define BCRYPT_RSA_ALGORITHM L"RSA"
#endif

#ifndef BCRYPT_DSA_ALGORITHM
#define BCRYPT_DSA_ALGORITHM L"DSA"
#endif

#ifndef BCRYPT_AES_ALGORITHM
#define BCRYPT_AES_ALGORITHM L"AES"
#endif

#ifndef BCRYPT_RC4_ALGORITHM
#define BCRYPT_RC4_ALGORITHM L"RC4"
#endif

#ifndef BCRYPT_3DES_ALGORITHM
#define BCRYPT_3DES_ALGORITHM L"3DES"
#endif

#ifndef BCRYPT_DH_ALGORITHM
#define BCRYPT_DH_ALGORITHM L"DH"
#endif

/* BCRYPT_KDF_RAW_SECRET is available from Windows 8.1 and onwards */
#ifndef BCRYPT_KDF_RAW_SECRET
#define BCRYPT_KDF_RAW_SECRET L"TRUNCATE"
#endif

#ifndef BCRYPT_ALG_HANDLE_HMAC_FLAG
#define BCRYPT_ALG_HANDLE_HMAC_FLAG 0x00000008
#endif

#ifndef BCRYPT_DSA_PUBLIC_BLOB
#define BCRYPT_DSA_PUBLIC_BLOB L"DSAPUBLICBLOB"
#endif

#ifndef BCRYPT_DSA_PUBLIC_MAGIC
#define BCRYPT_DSA_PUBLIC_MAGIC 0x42505344 /* DSPB */
#endif

#ifndef BCRYPT_DSA_PRIVATE_BLOB
#define BCRYPT_DSA_PRIVATE_BLOB L"DSAPRIVATEBLOB"
#endif

#ifndef BCRYPT_DSA_PRIVATE_MAGIC
#define BCRYPT_DSA_PRIVATE_MAGIC 0x56505344 /* DSPV */
#endif

#ifndef BCRYPT_RSAPUBLIC_BLOB
#define BCRYPT_RSAPUBLIC_BLOB L"RSAPUBLICBLOB"
#endif

#ifndef BCRYPT_RSAPUBLIC_MAGIC
#define BCRYPT_RSAPUBLIC_MAGIC 0x31415352 /* RSA1 */
#endif

#ifndef BCRYPT_RSAFULLPRIVATE_BLOB
#define BCRYPT_RSAFULLPRIVATE_BLOB L"RSAFULLPRIVATEBLOB"
#endif

#ifndef BCRYPT_RSAFULLPRIVATE_MAGIC
#define BCRYPT_RSAFULLPRIVATE_MAGIC 0x33415352 /* RSA3 */
#endif

#ifndef BCRYPT_KEY_DATA_BLOB
#define BCRYPT_KEY_DATA_BLOB L"KeyDataBlob"
#endif

#ifndef BCRYPT_MESSAGE_BLOCK_LENGTH
#define BCRYPT_MESSAGE_BLOCK_LENGTH L"MessageBlockLength"
#endif

#ifndef BCRYPT_NO_KEY_VALIDATION
#define BCRYPT_NO_KEY_VALIDATION 0x00000008
#endif

#ifndef BCRYPT_BLOCK_PADDING
#define BCRYPT_BLOCK_PADDING 0x00000001
#endif

#ifndef BCRYPT_PAD_NONE
#define BCRYPT_PAD_NONE 0x00000001
#endif

#ifndef BCRYPT_PAD_PKCS1
#define BCRYPT_PAD_PKCS1 0x00000002
#endif

#ifndef BCRYPT_PAD_OAEP
#define BCRYPT_PAD_OAEP 0x00000004
#endif

#ifndef BCRYPT_PAD_PSS
#define BCRYPT_PAD_PSS 0x00000008
#endif

#ifndef CRYPT_STRING_ANY
#define CRYPT_STRING_ANY 0x00000007
#endif

#ifndef LEGACY_RSAPRIVATE_BLOB
#define LEGACY_RSAPRIVATE_BLOB L"CAPIPRIVATEBLOB"
#endif

#ifndef PKCS_RSA_PRIVATE_KEY
#define PKCS_RSA_PRIVATE_KEY (LPCSTR)43
#endif

#if defined(_MSC_VER) && _MSC_VER < 1700
/* Workaround for warning C4306:
   'type cast' : conversion from 'int' to 'LPCSTR' of greater size */
#undef X509_SEQUENCE_OF_ANY
#undef X509_MULTI_BYTE_UINT
#undef PKCS_RSA_PRIVATE_KEY
#define X509_SEQUENCE_OF_ANY ((LPCSTR)(size_t)34)
#define X509_MULTI_BYTE_UINT ((LPCSTR)(size_t)38)
#define PKCS_RSA_PRIVATE_KEY ((LPCSTR)(size_t)43)
#endif

static int
_libssh2_wincng_bignum_resize(_libssh2_bn* bn, ULONG length);

/*******************************************************************/
/*
 * Windows CNG backend: ECDSA-specific declarations.
 */
#if LIBSSH2_ECDSA

typedef enum {
    WINCNG_ECC_KEYTYPE_ECDSA = 0,
    WINCNG_ECC_KEYTYPE_ECDH = 1,
} _libssh2_wincng_ecc_keytype;

typedef struct __libssh2_wincng_ecdsa_algorithm {
    /* Algorithm name */
    const char *name;

    /* Key length, in bits */
    ULONG key_length;

    /* Length of each point, in bytes */
    ULONG point_length;

    /* Name of CNG algorithm provider, */
    /* indexed by _libssh2_wincng_ecc_keytype */
    LPCWSTR provider[2];

    /* Magic for public key import, indexed by _libssh2_wincng_ecc_keytype */
    ULONG public_import_magic[2];

    /* Magic for private key import, indexed by _libssh2_wincng_ecc_keytype */
    ULONG private_import_magic[2];
} _libssh2_wincng_ecdsa_algorithm;

/* Supported algorithms, indexed by libssh2_curve_type */
static _libssh2_wincng_ecdsa_algorithm _wincng_ecdsa_algorithms[] = {
    {
        "ecdsa-sha2-nistp256",
        256,
        256 / 8,
        { BCRYPT_ECDSA_P256_ALGORITHM, BCRYPT_ECDH_P256_ALGORITHM },
        { BCRYPT_ECDSA_PUBLIC_P256_MAGIC, BCRYPT_ECDH_PUBLIC_P256_MAGIC },
        { BCRYPT_ECDSA_PRIVATE_P256_MAGIC, BCRYPT_ECDH_PRIVATE_P256_MAGIC }
    },
    {
        "ecdsa-sha2-nistp384",
        384,
        384 / 8,
        { BCRYPT_ECDSA_P384_ALGORITHM, BCRYPT_ECDH_P384_ALGORITHM },
        { BCRYPT_ECDSA_PUBLIC_P384_MAGIC, BCRYPT_ECDH_PUBLIC_P384_MAGIC },
        { BCRYPT_ECDSA_PRIVATE_P384_MAGIC, BCRYPT_ECDH_PRIVATE_P384_MAGIC }
    },
    {
        "ecdsa-sha2-nistp521",
        521,
        ((521 + 7) & ~7) / 8,
        { BCRYPT_ECDSA_P521_ALGORITHM, BCRYPT_ECDH_P521_ALGORITHM },
        { BCRYPT_ECDSA_PUBLIC_P521_MAGIC, BCRYPT_ECDH_PUBLIC_P521_MAGIC },
        { BCRYPT_ECDSA_PRIVATE_P521_MAGIC, BCRYPT_ECDH_PRIVATE_P521_MAGIC }
    },
};

/* An encoded point */
typedef struct __libssh2_ecdsa_point {
    libssh2_curve_type curve;

    const unsigned char *x;
    ULONG x_len;

    const unsigned char *y;
    ULONG y_len;
} _libssh2_ecdsa_point;

/* Lookup libssh2_curve_type by name */
static int
_libssh2_wincng_ecdsa_curve_type_from_name(IN const char *name,
                                           OUT libssh2_curve_type *out_curve);

/* Parse an OpenSSL-formatted ECDSA private key */
static int
_libssh2_wincng_parse_ecdsa_privatekey(OUT _libssh2_wincng_ecdsa_key **key,
                                       IN unsigned char *privatekey,
                                       IN size_t privatekey_len);

#endif

/*******************************************************************/
/*
 * Windows CNG backend: Generic functions
 */

struct _libssh2_wincng_ctx _libssh2_wincng;

void
_libssh2_wincng_init(void)
{
    int ret;

#if LIBSSH2_ECDSA
    unsigned int curve;
#endif

    memset(&_libssh2_wincng, 0, sizeof(_libssh2_wincng));

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgRNG,
                                      BCRYPT_RNG_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgRNG = NULL;
    }

#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHashMD5,
                                      BCRYPT_MD5_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHashMD5 = NULL;
    }
#endif
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHashSHA1,
                                      BCRYPT_SHA1_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHashSHA1 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHashSHA256,
                                      BCRYPT_SHA256_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHashSHA256 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHashSHA384,
                                      BCRYPT_SHA384_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHashSHA384 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHashSHA512,
                                      BCRYPT_SHA512_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHashSHA512 = NULL;
    }

#if LIBSSH2_MD5
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHmacMD5,
                                      BCRYPT_MD5_ALGORITHM, NULL,
                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHmacMD5 = NULL;
    }
#endif
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHmacSHA1,
                                      BCRYPT_SHA1_ALGORITHM, NULL,
                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHmacSHA1 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHmacSHA256,
                                      BCRYPT_SHA256_ALGORITHM, NULL,
                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHmacSHA256 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHmacSHA384,
                                      BCRYPT_SHA384_ALGORITHM, NULL,
                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHmacSHA384 = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgHmacSHA512,
                                      BCRYPT_SHA512_ALGORITHM, NULL,
                                      BCRYPT_ALG_HANDLE_HMAC_FLAG);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgHmacSHA512 = NULL;
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgRSA,
                                      BCRYPT_RSA_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgRSA = NULL;
    }
    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgDSA,
                                      BCRYPT_DSA_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgDSA = NULL;
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgAES_CBC,
                                      BCRYPT_AES_ALGORITHM, NULL, 0);
    if(BCRYPT_SUCCESS(ret)) {
        ret = BCryptSetProperty(_libssh2_wincng.hAlgAES_CBC,
                                BCRYPT_CHAINING_MODE,
                                (PBYTE)BCRYPT_CHAIN_MODE_CBC,
                                sizeof(BCRYPT_CHAIN_MODE_CBC), 0);
        if(!BCRYPT_SUCCESS(ret)) {
            ret = BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgAES_CBC, 0);
            if(BCRYPT_SUCCESS(ret)) {
                _libssh2_wincng.hAlgAES_CBC = NULL;
            }
        }
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgAES_ECB,
                                      BCRYPT_AES_ALGORITHM, NULL, 0);
    if(BCRYPT_SUCCESS(ret)) {
        ret = BCryptSetProperty(_libssh2_wincng.hAlgAES_ECB,
                                BCRYPT_CHAINING_MODE,
                                (PBYTE)BCRYPT_CHAIN_MODE_ECB,
                                sizeof(BCRYPT_CHAIN_MODE_ECB), 0);
        if(!BCRYPT_SUCCESS(ret)) {
            ret = BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgAES_ECB, 0);
            if(BCRYPT_SUCCESS(ret)) {
                _libssh2_wincng.hAlgAES_ECB = NULL;
            }
        }
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgRC4_NA,
                                      BCRYPT_RC4_ALGORITHM, NULL, 0);
    if(BCRYPT_SUCCESS(ret)) {
        ret = BCryptSetProperty(_libssh2_wincng.hAlgRC4_NA,
                                BCRYPT_CHAINING_MODE,
                                (PBYTE)BCRYPT_CHAIN_MODE_NA,
                                sizeof(BCRYPT_CHAIN_MODE_NA), 0);
        if(!BCRYPT_SUCCESS(ret)) {
            ret = BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgRC4_NA, 0);
            if(BCRYPT_SUCCESS(ret)) {
                _libssh2_wincng.hAlgRC4_NA = NULL;
            }
        }
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlg3DES_CBC,
                                      BCRYPT_3DES_ALGORITHM, NULL, 0);
    if(BCRYPT_SUCCESS(ret)) {
        ret = BCryptSetProperty(_libssh2_wincng.hAlg3DES_CBC,
                                BCRYPT_CHAINING_MODE,
                                (PBYTE)BCRYPT_CHAIN_MODE_CBC,
                                sizeof(BCRYPT_CHAIN_MODE_CBC), 0);
        if(!BCRYPT_SUCCESS(ret)) {
            ret = BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlg3DES_CBC,
                                               0);
            if(BCRYPT_SUCCESS(ret)) {
                _libssh2_wincng.hAlg3DES_CBC = NULL;
            }
        }
    }

    ret = BCryptOpenAlgorithmProvider(&_libssh2_wincng.hAlgDH,
                                      BCRYPT_DH_ALGORITHM, NULL, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng.hAlgDH = NULL;
    }

#if LIBSSH2_ECDSA
    for(curve = 0; curve < ARRAY_SIZE(_wincng_ecdsa_algorithms); curve++) {
        BCRYPT_ALG_HANDLE alg_handle_ecdsa;
        BCRYPT_ALG_HANDLE alg_handle_ecdh;

        ret = BCryptOpenAlgorithmProvider(
            &alg_handle_ecdsa,
            _wincng_ecdsa_algorithms[curve].provider[WINCNG_ECC_KEYTYPE_ECDSA],
            NULL,
            0);
        if(BCRYPT_SUCCESS(ret)) {
            _libssh2_wincng.hAlgECDSA[curve] = alg_handle_ecdsa;
        }

        ret = BCryptOpenAlgorithmProvider(
            &alg_handle_ecdh,
            _wincng_ecdsa_algorithms[curve].provider[WINCNG_ECC_KEYTYPE_ECDH],
            NULL,
            0);
        if(BCRYPT_SUCCESS(ret)) {
            _libssh2_wincng.hAlgECDH[curve] = alg_handle_ecdh;
        }
    }
#endif
}

void
_libssh2_wincng_free(void)
{
#if LIBSSH2_ECDSA
    unsigned int curve;
#endif

    if(_libssh2_wincng.hAlgRNG)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgRNG, 0);
#if LIBSSH2_MD5 || LIBSSH2_MD5_PEM
    if(_libssh2_wincng.hAlgHashMD5)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHashMD5, 0);
#endif
    if(_libssh2_wincng.hAlgHashSHA1)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHashSHA1, 0);
    if(_libssh2_wincng.hAlgHashSHA256)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHashSHA256, 0);
    if(_libssh2_wincng.hAlgHashSHA384)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHashSHA384, 0);
    if(_libssh2_wincng.hAlgHashSHA512)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHashSHA512, 0);
#if LIBSSH2_MD5
    if(_libssh2_wincng.hAlgHmacMD5)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHmacMD5, 0);
#endif
    if(_libssh2_wincng.hAlgHmacSHA1)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHmacSHA1, 0);
    if(_libssh2_wincng.hAlgHmacSHA256)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHmacSHA256, 0);
    if(_libssh2_wincng.hAlgHmacSHA384)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHmacSHA384, 0);
    if(_libssh2_wincng.hAlgHmacSHA512)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgHmacSHA512, 0);
    if(_libssh2_wincng.hAlgRSA)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgRSA, 0);
    if(_libssh2_wincng.hAlgDSA)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgDSA, 0);
    if(_libssh2_wincng.hAlgAES_CBC)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgAES_CBC, 0);
    if(_libssh2_wincng.hAlgRC4_NA)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgRC4_NA, 0);
    if(_libssh2_wincng.hAlg3DES_CBC)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlg3DES_CBC, 0);
    if(_libssh2_wincng.hAlgDH)
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgDH, 0);

#if LIBSSH2_ECDSA
    for(curve = 0; curve < ARRAY_SIZE(_wincng_ecdsa_algorithms); curve++) {
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgECDSA[curve],
                                           0);
        (void)BCryptCloseAlgorithmProvider(_libssh2_wincng.hAlgECDH[curve],
                                           0);
    }
#endif

    memset(&_libssh2_wincng, 0, sizeof(_libssh2_wincng));
}

int
_libssh2_wincng_random(void *buf, size_t len)
{
    int ret;

    if(len > ULONG_MAX) {
        return -1;
    }

    ret = BCryptGenRandom(_libssh2_wincng.hAlgRNG, buf, (ULONG)len, 0);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

static void
_libssh2_wincng_safe_free(void *buf, size_t len)
{
    if(!buf)
        return;

    if(len > 0)
        _libssh2_explicit_zero(buf, len);

    free(buf);
}

/* Copy a big endian set of bits from src to dest.
 * if the size of src is smaller than dest then pad the "left" (MSB)
 * end with zeroes and copy the bits into the "right" (LSB) end. */
static void
memcpy_with_be_padding(unsigned char *dest, ULONG dest_len,
                       unsigned char *src, ULONG src_len)
{
    if(dest_len > src_len) {
        memset(dest, 0, dest_len - src_len);
    }
    memcpy((dest + dest_len) - src_len, src, src_len);
}

/*******************************************************************/
/*
 * Windows CNG backend: Hash functions
 */

int
_libssh2_wincng_hash_init(_libssh2_wincng_hash_ctx *ctx,
                          BCRYPT_ALG_HANDLE hAlg, ULONG hashlen,
                          unsigned char *key, ULONG keylen)
{
    BCRYPT_HASH_HANDLE hHash;
    unsigned char *pbHashObject;
    ULONG dwHashObject, dwHash, cbData;
    int ret;

    ret = BCryptGetProperty(hAlg, BCRYPT_HASH_LENGTH,
                            (unsigned char *)&dwHash,
                            sizeof(dwHash),
                            &cbData, 0);
    if((!BCRYPT_SUCCESS(ret)) || dwHash != hashlen) {
        return -1;
    }

    ret = BCryptGetProperty(hAlg, BCRYPT_OBJECT_LENGTH,
                            (unsigned char *)&dwHashObject,
                            sizeof(dwHashObject),
                            &cbData, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        return -1;
    }

    pbHashObject = malloc(dwHashObject);
    if(!pbHashObject) {
        return -1;
    }


    ret = BCryptCreateHash(hAlg, &hHash,
                           pbHashObject, dwHashObject,
                           key, keylen, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng_safe_free(pbHashObject, dwHashObject);
        return -1;
    }


    ctx->hHash = hHash;
    ctx->pbHashObject = pbHashObject;
    ctx->dwHashObject = dwHashObject;
    ctx->cbHash = dwHash;

    return 0;
}

int
_libssh2_wincng_hash_update(_libssh2_wincng_hash_ctx *ctx,
                            const void *data, ULONG datalen)
{
    int ret;

    ret = BCryptHashData(ctx->hHash, (PUCHAR)data, datalen, 0);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

int
_libssh2_wincng_hash_final(_libssh2_wincng_hash_ctx *ctx,
                           unsigned char *hash)
{
    int ret;

    ret = BCryptFinishHash(ctx->hHash, hash, ctx->cbHash, 0);

    BCryptDestroyHash(ctx->hHash);
    ctx->hHash = NULL;

    _libssh2_wincng_safe_free(ctx->pbHashObject, ctx->dwHashObject);
    ctx->pbHashObject = NULL;
    ctx->dwHashObject = 0;

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

int
_libssh2_wincng_hash(const unsigned char *data, ULONG datalen,
                     BCRYPT_ALG_HANDLE hAlg,
                     unsigned char *hash, ULONG hashlen)
{
    _libssh2_wincng_hash_ctx ctx;
    int ret;

    ret = _libssh2_wincng_hash_init(&ctx, hAlg, hashlen, NULL, 0);
    if(!ret) {
        ret = _libssh2_wincng_hash_update(&ctx, data, datalen);
        ret |= _libssh2_wincng_hash_final(&ctx, hash);
    }

    return ret;
}


/*******************************************************************/
/*
 * Windows CNG backend: HMAC functions
 */

int _libssh2_hmac_ctx_init(libssh2_hmac_ctx *ctx)
{
    memset(ctx, 0, sizeof(*ctx));
    return 1;
}

#if LIBSSH2_MD5
int _libssh2_hmac_md5_init(libssh2_hmac_ctx *ctx,
                           void *key, size_t keylen)
{
    int ret = _libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHmacMD5,
                                        MD5_DIGEST_LENGTH,
                                        key, (ULONG) keylen);

    return ret == 0 ? 1 : 0;
}
#endif

int _libssh2_hmac_sha1_init(libssh2_hmac_ctx *ctx,
                            void *key, size_t keylen)
{
    int ret = _libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHmacSHA1,
                                        SHA_DIGEST_LENGTH,
                                        key, (ULONG) keylen);

    return ret == 0 ? 1 : 0;
}

int _libssh2_hmac_sha256_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    int ret = _libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHmacSHA256,
                                        SHA256_DIGEST_LENGTH,
                                        key, (ULONG) keylen);

    return ret == 0 ? 1 : 0;
}

int _libssh2_hmac_sha512_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    int ret = _libssh2_wincng_hash_init(ctx, _libssh2_wincng.hAlgHmacSHA512,
                                        SHA512_DIGEST_LENGTH,
                                        key, (ULONG) keylen);

    return ret == 0 ? 1 : 0;
}

int _libssh2_hmac_update(libssh2_hmac_ctx *ctx,
                         const void *data, size_t datalen)
{
    int ret = _libssh2_wincng_hash_update(ctx, data, (ULONG) datalen);

    return ret == 0 ? 1 : 0;
}

int _libssh2_hmac_final(libssh2_hmac_ctx *ctx, void *data)
{
    int ret = BCryptFinishHash(ctx->hHash, data, ctx->cbHash, 0);

    return BCRYPT_SUCCESS(ret) ? 1 : 0;
}

void _libssh2_hmac_cleanup(libssh2_hmac_ctx *ctx)
{
    BCryptDestroyHash(ctx->hHash);
    ctx->hHash = NULL;

    _libssh2_wincng_safe_free(ctx->pbHashObject, ctx->dwHashObject);
    ctx->pbHashObject = NULL;
    ctx->dwHashObject = 0;
}


/*******************************************************************/
/*
 * Windows CNG backend: Key functions
 */

static int
_libssh2_wincng_key_sha_verify(_libssh2_wincng_key_ctx *ctx,
                               ULONG hashlen,
                               const unsigned char *sig,
                               ULONG sig_len,
                               const unsigned char *m,
                               ULONG m_len,
                               ULONG flags)
{
    BCRYPT_PKCS1_PADDING_INFO paddingInfoPKCS1;
    BCRYPT_ALG_HANDLE hAlgHash;
    void *pPaddingInfo;
    unsigned char *data, *hash;
    ULONG datalen;
    int ret;

    if(hashlen == SHA_DIGEST_LENGTH) {
        hAlgHash = _libssh2_wincng.hAlgHashSHA1;
        paddingInfoPKCS1.pszAlgId = BCRYPT_SHA1_ALGORITHM;
    }
    else if(hashlen == SHA256_DIGEST_LENGTH) {
        hAlgHash = _libssh2_wincng.hAlgHashSHA256;
        paddingInfoPKCS1.pszAlgId = BCRYPT_SHA256_ALGORITHM;
    }
    else if(hashlen == SHA384_DIGEST_LENGTH) {
        hAlgHash = _libssh2_wincng.hAlgHashSHA384;
        paddingInfoPKCS1.pszAlgId = BCRYPT_SHA384_ALGORITHM;
    }
    else if(hashlen == SHA512_DIGEST_LENGTH) {
        hAlgHash = _libssh2_wincng.hAlgHashSHA512;
        paddingInfoPKCS1.pszAlgId = BCRYPT_SHA512_ALGORITHM;
    }
    else {
        return -1;
    }

    datalen = m_len;
    data = malloc(datalen);
    if(!data) {
        return -1;
    }

    hash = malloc(hashlen);
    if(!hash) {
        free(data);
        return -1;
    }
    memcpy(data, m, datalen);

    ret = _libssh2_wincng_hash(data, datalen,
                               hAlgHash,
                               hash, hashlen);
    _libssh2_wincng_safe_free(data, datalen);

    if(ret) {
        _libssh2_wincng_safe_free(hash, hashlen);
        return -1;
    }

    datalen = sig_len;
    data = malloc(datalen);
    if(!data) {
        _libssh2_wincng_safe_free(hash, hashlen);
        return -1;
    }

    if(flags & BCRYPT_PAD_PKCS1) {
        pPaddingInfo = &paddingInfoPKCS1;
    }
    else
        pPaddingInfo = NULL;

    memcpy(data, sig, datalen);

    ret = BCryptVerifySignature(ctx->hKey, pPaddingInfo,
                                hash, hashlen, data, datalen, flags);

    _libssh2_wincng_safe_free(hash, hashlen);
    _libssh2_wincng_safe_free(data, datalen);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

#ifdef HAVE_LIBCRYPT32
static int
_libssh2_wincng_load_pem(LIBSSH2_SESSION *session,
                         const char *filename,
                         const unsigned char *passphrase,
                         const char *headerbegin,
                         const char *headerend,
                         unsigned char **data,
                         size_t *datalen)
{
    FILE *fp;
    int ret;

    fp = fopen(filename, FOPEN_READTEXT);
    if(!fp) {
        return -1;
    }

    ret = _libssh2_pem_parse(session, headerbegin, headerend,
                             passphrase,
                             fp, data, datalen);

    fclose(fp);

    return ret;
}

static int
_libssh2_wincng_load_private(LIBSSH2_SESSION *session,
                             const char *filename,
                             const unsigned char *passphrase,
                             unsigned char **ppbEncoded,
                             size_t *pcbEncoded,
                             int tryLoadRSA, int tryLoadDSA)
{
    unsigned char *data = NULL;
    size_t datalen = 0;
    int ret = -1;

    if(ret && tryLoadRSA) {
        ret = _libssh2_wincng_load_pem(session, filename, passphrase,
                                       PEM_RSA_HEADER, PEM_RSA_FOOTER,
                                       &data, &datalen);
    }

    if(ret && tryLoadDSA) {
        ret = _libssh2_wincng_load_pem(session, filename, passphrase,
                                       PEM_DSA_HEADER, PEM_DSA_FOOTER,
                                       &data, &datalen);
    }

    if(!ret) {
        *ppbEncoded = data;
        *pcbEncoded = datalen;
    }

    return ret;
}

static int
_libssh2_wincng_load_private_memory(LIBSSH2_SESSION *session,
                                    const char *privatekeydata,
                                    size_t privatekeydata_len,
                                    const unsigned char *passphrase,
                                    unsigned char **ppbEncoded,
                                    size_t *pcbEncoded,
                                    int tryLoadRSA, int tryLoadDSA)
{
    unsigned char *data = NULL;
    size_t datalen = 0;
    int ret = -1;

    (void)passphrase;

    if(ret && tryLoadRSA) {
        ret = _libssh2_pem_parse_memory(session,
                                        PEM_RSA_HEADER, PEM_RSA_FOOTER,
                                        privatekeydata, privatekeydata_len,
                                        &data, &datalen);
    }

    if(ret && tryLoadDSA) {
        ret = _libssh2_pem_parse_memory(session,
                                        PEM_DSA_HEADER, PEM_DSA_FOOTER,
                                        privatekeydata, privatekeydata_len,
                                        &data, &datalen);
    }

    if(!ret) {
        *ppbEncoded = data;
        *pcbEncoded = datalen;
    }

    return ret;
}

static int
_libssh2_wincng_asn_decode(unsigned char *pbEncoded,
                           DWORD cbEncoded,
                           LPCSTR lpszStructType,
                           unsigned char **ppbDecoded,
                           DWORD *pcbDecoded)
{
    unsigned char *pbDecoded = NULL;
    DWORD cbDecoded = 0;
    int ret;

    ret = CryptDecodeObjectEx(X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                              lpszStructType,
                              pbEncoded, cbEncoded, 0, NULL,
                              NULL, &cbDecoded);
    if(!ret) {
        return -1;
    }

    pbDecoded = malloc(cbDecoded);
    if(!pbDecoded) {
        return -1;
    }

    ret = CryptDecodeObjectEx(X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
                              lpszStructType,
                              pbEncoded, cbEncoded, 0, NULL,
                              pbDecoded, &cbDecoded);
    if(!ret) {
        _libssh2_wincng_safe_free(pbDecoded, cbDecoded);
        return -1;
    }


    *ppbDecoded = pbDecoded;
    *pcbDecoded = cbDecoded;

    return 0;
}

static int
_libssh2_wincng_bn_ltob(unsigned char *pbInput,
                        DWORD cbInput,
                        unsigned char **ppbOutput,
                        DWORD *pcbOutput)
{
    unsigned char *pbOutput;
    DWORD cbOutput, index, offset, length;

    if(cbInput < 1) {
        return 0;
    }

    offset = 0;
    length = cbInput - 1;
    cbOutput = cbInput;
    if(pbInput[length] & (1 << 7)) {
        offset++;
        cbOutput += offset;
    }

    pbOutput = (unsigned char *)malloc(cbOutput);
    if(!pbOutput) {
        return -1;
    }

    pbOutput[0] = 0;
    for(index = 0; ((index + offset) < cbOutput)
                    && (index < cbInput); index++) {
        pbOutput[index + offset] = pbInput[length - index];
    }


    *ppbOutput = pbOutput;
    *pcbOutput = cbOutput;

    return 0;
}

static int
_libssh2_wincng_asn_decode_bn(unsigned char *pbEncoded,
                              DWORD cbEncoded,
                              unsigned char **ppbDecoded,
                              DWORD *pcbDecoded)
{
    unsigned char *pbDecoded = NULL;
    PCRYPT_DATA_BLOB pbInteger;
    DWORD cbDecoded = 0, cbInteger;
    int ret;

    ret = _libssh2_wincng_asn_decode(pbEncoded, cbEncoded,
                                     X509_MULTI_BYTE_UINT,
                                     (void *)&pbInteger, &cbInteger);
    if(!ret) {
        ret = _libssh2_wincng_bn_ltob(pbInteger->pbData,
                                      pbInteger->cbData,
                                      &pbDecoded, &cbDecoded);
        if(!ret) {
            *ppbDecoded = pbDecoded;
            *pcbDecoded = cbDecoded;
        }
        _libssh2_wincng_safe_free(pbInteger, cbInteger);
    }

    return ret;
}

static int
_libssh2_wincng_asn_decode_bns(unsigned char *pbEncoded,
                               DWORD cbEncoded,
                               unsigned char ***prpbDecoded,
                               DWORD **prcbDecoded,
                               DWORD *pcbCount)
{
    PCRYPT_DER_BLOB pBlob;
    unsigned char **rpbDecoded;
    PCRYPT_SEQUENCE_OF_ANY pbDecoded;
    DWORD cbDecoded, *rcbDecoded, index, length;
    int ret;

    ret = _libssh2_wincng_asn_decode(pbEncoded, cbEncoded,
                                     X509_SEQUENCE_OF_ANY,
                                     (void *)&pbDecoded, &cbDecoded);
    if(!ret) {
        length = pbDecoded->cValue;

        rpbDecoded = malloc(sizeof(PBYTE) * length);
        if(rpbDecoded) {
            rcbDecoded = malloc(sizeof(DWORD) * length);
            if(rcbDecoded) {
                for(index = 0; index < length; index++) {
                    pBlob = &pbDecoded->rgValue[index];
                    ret = _libssh2_wincng_asn_decode_bn(pBlob->pbData,
                                                        pBlob->cbData,
                                                        &rpbDecoded[index],
                                                        &rcbDecoded[index]);
                    if(ret)
                        break;
                }

                if(!ret) {
                    *prpbDecoded = rpbDecoded;
                    *prcbDecoded = rcbDecoded;
                    *pcbCount = length;
                }
                else {
                    for(length = 0; length < index; length++) {
                        _libssh2_wincng_safe_free(rpbDecoded[length],
                                                  rcbDecoded[length]);
                        rpbDecoded[length] = NULL;
                        rcbDecoded[length] = 0;
                    }
                    free(rpbDecoded);
                    free(rcbDecoded);
                }
            }
            else {
                free(rpbDecoded);
                ret = -1;
            }
        }
        else {
            ret = -1;
        }

        _libssh2_wincng_safe_free(pbDecoded, cbDecoded);
    }

    return ret;
}
#endif /* HAVE_LIBCRYPT32 */

#if LIBSSH2_RSA || LIBSSH2_DSA
static ULONG
_libssh2_wincng_bn_size(const unsigned char *bignum, ULONG length)
{
    ULONG offset;

    if(!bignum)
        return 0;

    length--;

    offset = 0;
    while(!(*(bignum + offset)) && (offset < length))
        offset++;

    length++;

    return length - offset;
}
#endif


#if LIBSSH2_RSA
/*******************************************************************/
/*
 * Windows CNG backend: RSA functions
 */

int
_libssh2_wincng_rsa_new(libssh2_rsa_ctx **rsa,
                        const unsigned char *edata,
                        unsigned long elen,
                        const unsigned char *ndata,
                        unsigned long nlen,
                        const unsigned char *ddata,
                        unsigned long dlen,
                        const unsigned char *pdata,
                        unsigned long plen,
                        const unsigned char *qdata,
                        unsigned long qlen,
                        const unsigned char *e1data,
                        unsigned long e1len,
                        const unsigned char *e2data,
                        unsigned long e2len,
                        const unsigned char *coeffdata,
                        unsigned long coefflen)
{
    BCRYPT_KEY_HANDLE hKey;
    BCRYPT_RSAKEY_BLOB *rsakey;
    LPCWSTR lpszBlobType;
    ULONG keylen, offset, mlen, p1len = 0, p2len = 0;
    int ret;

    mlen = max(_libssh2_wincng_bn_size(ndata, nlen),
               _libssh2_wincng_bn_size(ddata, dlen));
    offset = sizeof(BCRYPT_RSAKEY_BLOB);
    keylen = offset + elen + mlen;
    if(ddata && dlen > 0) {
        p1len = max(_libssh2_wincng_bn_size(pdata, plen),
                    _libssh2_wincng_bn_size(e1data, e1len));
        p2len = max(_libssh2_wincng_bn_size(qdata, qlen),
                    _libssh2_wincng_bn_size(e2data, e2len));
        keylen += p1len * 3 + p2len * 2 + mlen;
    }

    rsakey = (BCRYPT_RSAKEY_BLOB *)malloc(keylen);
    if(!rsakey) {
        return -1;
    }

    memset(rsakey, 0, keylen);


    /* https://msdn.microsoft.com/library/windows/desktop/aa375531.aspx */
    rsakey->BitLength = mlen * 8;
    rsakey->cbPublicExp = elen;
    rsakey->cbModulus = mlen;

    memcpy((unsigned char *)rsakey + offset, edata, elen);
    offset += elen;

    if(nlen < mlen)
        memcpy((unsigned char *)rsakey + offset + mlen - nlen, ndata, nlen);
    else
        memcpy((unsigned char *)rsakey + offset, ndata + nlen - mlen, mlen);

    if(ddata && dlen > 0) {
        offset += mlen;

        if(plen < p1len)
            memcpy((unsigned char *)rsakey + offset + p1len - plen,
                   pdata, plen);
        else
            memcpy((unsigned char *)rsakey + offset,
                   pdata + plen - p1len, p1len);
        offset += p1len;

        if(qlen < p2len)
            memcpy((unsigned char *)rsakey + offset + p2len - qlen,
                   qdata, qlen);
        else
            memcpy((unsigned char *)rsakey + offset,
                   qdata + qlen - p2len, p2len);
        offset += p2len;

        if(e1len < p1len)
            memcpy((unsigned char *)rsakey + offset + p1len - e1len,
                   e1data, e1len);
        else
            memcpy((unsigned char *)rsakey + offset,
                   e1data + e1len - p1len, p1len);
        offset += p1len;

        if(e2len < p2len)
            memcpy((unsigned char *)rsakey + offset + p2len - e2len,
                   e2data, e2len);
        else
            memcpy((unsigned char *)rsakey + offset,
                   e2data + e2len - p2len, p2len);
        offset += p2len;

        if(coefflen < p1len)
            memcpy((unsigned char *)rsakey + offset + p1len - coefflen,
                   coeffdata, coefflen);
        else
            memcpy((unsigned char *)rsakey + offset,
                   coeffdata + coefflen - p1len, p1len);
        offset += p1len;

        if(dlen < mlen)
            memcpy((unsigned char *)rsakey + offset + mlen - dlen,
                   ddata, dlen);
        else
            memcpy((unsigned char *)rsakey + offset,
                   ddata + dlen - mlen, mlen);

        lpszBlobType = BCRYPT_RSAFULLPRIVATE_BLOB;
        rsakey->Magic = BCRYPT_RSAFULLPRIVATE_MAGIC;
        rsakey->cbPrime1 = p1len;
        rsakey->cbPrime2 = p2len;
    }
    else {
        lpszBlobType = BCRYPT_RSAPUBLIC_BLOB;
        rsakey->Magic = BCRYPT_RSAPUBLIC_MAGIC;
        rsakey->cbPrime1 = 0;
        rsakey->cbPrime2 = 0;
    }


    ret = BCryptImportKeyPair(_libssh2_wincng.hAlgRSA, NULL, lpszBlobType,
                              &hKey, (PUCHAR)rsakey, keylen, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng_safe_free(rsakey, keylen);
        return -1;
    }


    *rsa = malloc(sizeof(libssh2_rsa_ctx));
    if(!(*rsa)) {
        BCryptDestroyKey(hKey);
        _libssh2_wincng_safe_free(rsakey, keylen);
        return -1;
    }

    (*rsa)->hKey = hKey;
    (*rsa)->pbKeyObject = rsakey;
    (*rsa)->cbKeyObject = keylen;

    return 0;
}

#ifdef HAVE_LIBCRYPT32
static int
_libssh2_wincng_rsa_new_private_parse(libssh2_rsa_ctx **rsa,
                                      LIBSSH2_SESSION *session,
                                      unsigned char *pbEncoded,
                                      size_t cbEncoded)
{
    BCRYPT_KEY_HANDLE hKey;
    unsigned char *pbStructInfo;
    DWORD cbStructInfo;
    int ret;

    (void)session;

    ret = _libssh2_wincng_asn_decode(pbEncoded, (DWORD)cbEncoded,
                                     PKCS_RSA_PRIVATE_KEY,
                                     &pbStructInfo, &cbStructInfo);

    _libssh2_wincng_safe_free(pbEncoded, cbEncoded);

    if(ret) {
        return -1;
    }


    ret = BCryptImportKeyPair(_libssh2_wincng.hAlgRSA, NULL,
                              LEGACY_RSAPRIVATE_BLOB, &hKey,
                              pbStructInfo, cbStructInfo, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng_safe_free(pbStructInfo, cbStructInfo);
        return -1;
    }


    *rsa = malloc(sizeof(libssh2_rsa_ctx));
    if(!(*rsa)) {
        BCryptDestroyKey(hKey);
        _libssh2_wincng_safe_free(pbStructInfo, cbStructInfo);
        return -1;
    }

    (*rsa)->hKey = hKey;
    (*rsa)->pbKeyObject = pbStructInfo;
    (*rsa)->cbKeyObject = cbStructInfo;

    return 0;
}
#endif /* HAVE_LIBCRYPT32 */

int
_libssh2_wincng_rsa_new_private(libssh2_rsa_ctx **rsa,
                                LIBSSH2_SESSION *session,
                                const char *filename,
                                const unsigned char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    (void)session;

    ret = _libssh2_wincng_load_private(session, filename, passphrase,
                                       &pbEncoded, &cbEncoded, 1, 0);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_rsa_new_private_parse(rsa, session,
                                                 pbEncoded, cbEncoded);
#else
    (void)rsa;
    (void)filename;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                          "Unable to load RSA key from private key file: "
                          "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

int
_libssh2_wincng_rsa_new_private_frommemory(libssh2_rsa_ctx **rsa,
                                           LIBSSH2_SESSION *session,
                                           const char *filedata,
                                           size_t filedata_len,
                                           const unsigned char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    (void)session;

    ret = _libssh2_wincng_load_private_memory(session, filedata, filedata_len,
                                              passphrase,
                                              &pbEncoded, &cbEncoded, 1, 0);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_rsa_new_private_parse(rsa, session,
                                                 pbEncoded, cbEncoded);
#else
    (void)rsa;
    (void)filedata;
    (void)filedata_len;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_METHOD_NOT_SUPPORTED,
                          "Unable to extract private key from memory: "
                          "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

#if LIBSSH2_RSA_SHA1
int
_libssh2_wincng_rsa_sha1_verify(libssh2_rsa_ctx *rsa,
                                const unsigned char *sig,
                                size_t sig_len,
                                const unsigned char *m,
                                size_t m_len)
{
    return _libssh2_wincng_key_sha_verify(rsa, SHA_DIGEST_LENGTH,
                                          sig, (ULONG)sig_len,
                                          m, (ULONG)m_len,
                                          BCRYPT_PAD_PKCS1);
}
#endif

#if LIBSSH2_RSA_SHA2
int
_libssh2_wincng_rsa_sha2_verify(libssh2_rsa_ctx *rsa,
                                size_t hash_len,
                                const unsigned char *sig,
                                size_t sig_len,
                                const unsigned char *m,
                                size_t m_len)
{
    return _libssh2_wincng_key_sha_verify(rsa, (ULONG)hash_len,
                                          sig, (ULONG)sig_len,
                                          m, (ULONG)m_len,
                                          BCRYPT_PAD_PKCS1);
}
#endif

static int
_libssh2_wincng_rsa_sha_sign(LIBSSH2_SESSION *session,
                             libssh2_rsa_ctx *rsa,
                             const unsigned char *hash,
                             size_t hash_len,
                             unsigned char **signature,
                             size_t *signature_len)
{
    BCRYPT_PKCS1_PADDING_INFO paddingInfo;
    unsigned char *data, *sig;
    ULONG cbData, datalen, siglen;
    NTSTATUS ret;

    if(hash_len == SHA_DIGEST_LENGTH)
        paddingInfo.pszAlgId = BCRYPT_SHA1_ALGORITHM;
    else if(hash_len == SHA256_DIGEST_LENGTH)
        paddingInfo.pszAlgId = BCRYPT_SHA256_ALGORITHM;
    else if(hash_len == SHA384_DIGEST_LENGTH)
        paddingInfo.pszAlgId = BCRYPT_SHA384_ALGORITHM;
    else if(hash_len == SHA512_DIGEST_LENGTH)
        paddingInfo.pszAlgId = BCRYPT_SHA512_ALGORITHM;
    else {
        _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                       "Unsupported hash digest length");
        return -1;
    }

    datalen = (ULONG)hash_len;
    data = malloc(datalen);
    if(!data) {
        return -1;
    }
    memcpy(data, hash, datalen);

    ret = BCryptSignHash(rsa->hKey, &paddingInfo,
                         data, datalen, NULL, 0,
                         &cbData, BCRYPT_PAD_PKCS1);
    if(BCRYPT_SUCCESS(ret)) {
        siglen = cbData;
        sig = LIBSSH2_ALLOC(session, siglen);
        if(sig) {
            ret = BCryptSignHash(rsa->hKey, &paddingInfo,
                                 data, datalen, sig, siglen,
                                 &cbData, BCRYPT_PAD_PKCS1);
            if(BCRYPT_SUCCESS(ret)) {
                *signature_len = siglen;
                *signature = sig;
            }
            else {
                LIBSSH2_FREE(session, sig);
            }
        }
        else
            ret = (NTSTATUS)STATUS_NO_MEMORY;
    }

    _libssh2_wincng_safe_free(data, datalen);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

int
_libssh2_wincng_rsa_sha1_sign(LIBSSH2_SESSION *session,
                              libssh2_rsa_ctx *rsa,
                              const unsigned char *hash,
                              size_t hash_len,
                              unsigned char **signature,
                              size_t *signature_len)
{
    return _libssh2_wincng_rsa_sha_sign(session, rsa,
                                        hash, hash_len,
                                        signature, signature_len);
}

int
_libssh2_wincng_rsa_sha2_sign(LIBSSH2_SESSION *session,
                              libssh2_rsa_ctx *rsa,
                              const unsigned char *hash,
                              size_t hash_len,
                              unsigned char **signature,
                              size_t *signature_len)
{
    return _libssh2_wincng_rsa_sha_sign(session, rsa,
                                        hash, hash_len,
                                        signature, signature_len);
}

void
_libssh2_wincng_rsa_free(libssh2_rsa_ctx *rsa)
{
    if(!rsa)
        return;

    BCryptDestroyKey(rsa->hKey);
    rsa->hKey = NULL;

    _libssh2_wincng_safe_free(rsa->pbKeyObject, rsa->cbKeyObject);
    _libssh2_wincng_safe_free(rsa, sizeof(libssh2_rsa_ctx));
}
#endif

/*******************************************************************/
/*
 * Windows CNG backend: DSA functions
 */

#if LIBSSH2_DSA
int
_libssh2_wincng_dsa_new(libssh2_dsa_ctx **dsa,
                        const unsigned char *pdata,
                        unsigned long plen,
                        const unsigned char *qdata,
                        unsigned long qlen,
                        const unsigned char *gdata,
                        unsigned long glen,
                        const unsigned char *ydata,
                        unsigned long ylen,
                        const unsigned char *xdata,
                        unsigned long xlen)
{
    BCRYPT_KEY_HANDLE hKey;
    BCRYPT_DSA_KEY_BLOB *dsakey;
    LPCWSTR lpszBlobType;
    ULONG keylen, offset, length;
    int ret;

    length = max(max(_libssh2_wincng_bn_size(pdata, plen),
                     _libssh2_wincng_bn_size(gdata, glen)),
                 _libssh2_wincng_bn_size(ydata, ylen));
    offset = sizeof(BCRYPT_DSA_KEY_BLOB);
    keylen = offset + length * 3;
    if(xdata && xlen > 0)
        keylen += 20;

    dsakey = (BCRYPT_DSA_KEY_BLOB *)malloc(keylen);
    if(!dsakey) {
        return -1;
    }

    memset(dsakey, 0, keylen);


    /* https://msdn.microsoft.com/library/windows/desktop/aa833126.aspx */
    dsakey->cbKey = length;

    memset(dsakey->Count, -1, sizeof(dsakey->Count));
    memset(dsakey->Seed, -1, sizeof(dsakey->Seed));

    if(qlen < 20)
        memcpy(dsakey->q + 20 - qlen, qdata, qlen);
    else
        memcpy(dsakey->q, qdata + qlen - 20, 20);

    if(plen < length)
        memcpy((unsigned char *)dsakey + offset + length - plen,
               pdata, plen);
    else
        memcpy((unsigned char *)dsakey + offset,
               pdata + plen - length, length);
    offset += length;

    if(glen < length)
        memcpy((unsigned char *)dsakey + offset + length - glen,
               gdata, glen);
    else
        memcpy((unsigned char *)dsakey + offset,
               gdata + glen - length, length);
    offset += length;

    if(ylen < length)
        memcpy((unsigned char *)dsakey + offset + length - ylen,
               ydata, ylen);
    else
        memcpy((unsigned char *)dsakey + offset,
               ydata + ylen - length, length);

    if(xdata && xlen > 0) {
        offset += length;

        if(xlen < 20)
            memcpy((unsigned char *)dsakey + offset + 20 - xlen, xdata, xlen);
        else
            memcpy((unsigned char *)dsakey + offset, xdata + xlen - 20, 20);

        lpszBlobType = BCRYPT_DSA_PRIVATE_BLOB;
        dsakey->dwMagic = BCRYPT_DSA_PRIVATE_MAGIC;
    }
    else {
        lpszBlobType = BCRYPT_DSA_PUBLIC_BLOB;
        dsakey->dwMagic = BCRYPT_DSA_PUBLIC_MAGIC;
    }


    ret = BCryptImportKeyPair(_libssh2_wincng.hAlgDSA, NULL, lpszBlobType,
                              &hKey, (PUCHAR)dsakey, keylen, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng_safe_free(dsakey, keylen);
        return -1;
    }


    *dsa = malloc(sizeof(libssh2_dsa_ctx));
    if(!(*dsa)) {
        BCryptDestroyKey(hKey);
        _libssh2_wincng_safe_free(dsakey, keylen);
        return -1;
    }

    (*dsa)->hKey = hKey;
    (*dsa)->pbKeyObject = dsakey;
    (*dsa)->cbKeyObject = keylen;

    return 0;
}

#ifdef HAVE_LIBCRYPT32
static int
_libssh2_wincng_dsa_new_private_parse(libssh2_dsa_ctx **dsa,
                                      LIBSSH2_SESSION *session,
                                      unsigned char *pbEncoded,
                                      size_t cbEncoded)
{
    unsigned char **rpbDecoded;
    DWORD *rcbDecoded, index, length;
    int ret;

    (void)session;

    ret = _libssh2_wincng_asn_decode_bns(pbEncoded, (DWORD)cbEncoded,
                                         &rpbDecoded, &rcbDecoded, &length);

    _libssh2_wincng_safe_free(pbEncoded, cbEncoded);

    if(ret) {
        return -1;
    }


    if(length == 6) {
        ret = _libssh2_wincng_dsa_new(dsa,
                                      rpbDecoded[1], rcbDecoded[1],
                                      rpbDecoded[2], rcbDecoded[2],
                                      rpbDecoded[3], rcbDecoded[3],
                                      rpbDecoded[4], rcbDecoded[4],
                                      rpbDecoded[5], rcbDecoded[5]);
    }
    else {
        ret = -1;
    }

    for(index = 0; index < length; index++) {
        _libssh2_wincng_safe_free(rpbDecoded[index], rcbDecoded[index]);
        rpbDecoded[index] = NULL;
        rcbDecoded[index] = 0;
    }

    free(rpbDecoded);
    free(rcbDecoded);

    return ret;
}
#endif /* HAVE_LIBCRYPT32 */

int
_libssh2_wincng_dsa_new_private(libssh2_dsa_ctx **dsa,
                                LIBSSH2_SESSION *session,
                                const char *filename,
                                const unsigned char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    ret = _libssh2_wincng_load_private(session, filename, passphrase,
                                       &pbEncoded, &cbEncoded, 0, 1);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_dsa_new_private_parse(dsa, session,
                                                 pbEncoded, cbEncoded);
#else
    (void)dsa;
    (void)filename;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                          "Unable to load DSA key from private key file: "
                          "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

int
_libssh2_wincng_dsa_new_private_frommemory(libssh2_dsa_ctx **dsa,
                                           LIBSSH2_SESSION *session,
                                           const char *filedata,
                                           size_t filedata_len,
                                           const unsigned char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    ret = _libssh2_wincng_load_private_memory(session, filedata, filedata_len,
                                              passphrase,
                                              &pbEncoded, &cbEncoded, 0, 1);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_dsa_new_private_parse(dsa, session,
                                                 pbEncoded, cbEncoded);
#else
    (void)dsa;
    (void)filedata;
    (void)filedata_len;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_METHOD_NOT_SUPPORTED,
                          "Unable to extract private key from memory: "
                          "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

int
_libssh2_wincng_dsa_sha1_verify(libssh2_dsa_ctx *dsa,
                                const unsigned char *sig_fixed,
                                const unsigned char *m,
                                size_t m_len)
{
    return _libssh2_wincng_key_sha_verify(dsa, SHA_DIGEST_LENGTH, sig_fixed,
                                          40, m, (ULONG)m_len, 0);
}

int
_libssh2_wincng_dsa_sha1_sign(libssh2_dsa_ctx *dsa,
                              const unsigned char *hash,
                              size_t hash_len,
                              unsigned char *sig_fixed)
{
    unsigned char *data, *sig;
    ULONG cbData, datalen, siglen;
    NTSTATUS ret;

    datalen = (ULONG)hash_len;
    data = malloc(datalen);
    if(!data) {
        return -1;
    }

    memcpy(data, hash, datalen);

    ret = BCryptSignHash(dsa->hKey, NULL, data, datalen,
                         NULL, 0, &cbData, 0);
    if(BCRYPT_SUCCESS(ret)) {
        siglen = cbData;
        if(siglen == 40) {
            sig = malloc(siglen);
            if(sig) {
                ret = BCryptSignHash(dsa->hKey, NULL, data, datalen,
                                     sig, siglen, &cbData, 0);
                if(BCRYPT_SUCCESS(ret)) {
                    memcpy(sig_fixed, sig, siglen);
                }

                _libssh2_wincng_safe_free(sig, siglen);
            }
            else
                ret = (NTSTATUS)STATUS_NO_MEMORY;
        }
        else
            ret = (NTSTATUS)STATUS_NO_MEMORY;
    }

    _libssh2_wincng_safe_free(data, datalen);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

void
_libssh2_wincng_dsa_free(libssh2_dsa_ctx *dsa)
{
    if(!dsa)
        return;

    BCryptDestroyKey(dsa->hKey);
    dsa->hKey = NULL;

    _libssh2_wincng_safe_free(dsa->pbKeyObject, dsa->cbKeyObject);
    _libssh2_wincng_safe_free(dsa, sizeof(libssh2_dsa_ctx));
}
#endif


/*******************************************************************/
/*
 * Windows CNG backend: ECDSA helper functions
 */

#if LIBSSH2_ECDSA

/*
 * Decode an uncompressed point.
 */
static int
_libssh2_wincng_ecdsa_decode_uncompressed_point(
    IN const unsigned char *encoded_point,
    IN size_t encoded_point_len,
    OUT _libssh2_ecdsa_point *point)
{
    unsigned int curve;

    if(!point) {
        return LIBSSH2_ERROR_INVAL;
    }

    /* Verify that the point uses uncompressed format */
    if(encoded_point_len == 0 || encoded_point[0] != 4) {
        return LIBSSH2_ERROR_INVAL;
    }

    for(curve = 0; curve < ARRAY_SIZE(_wincng_ecdsa_algorithms); curve++) {
        if(_wincng_ecdsa_algorithms[curve].point_length ==
            (encoded_point_len - 1) / 2) {

            point->curve = curve;

            point->x = encoded_point + 1;
            point->x_len = _wincng_ecdsa_algorithms[curve].point_length;

            point->y = point->x + point->x_len;
            point->y_len = _wincng_ecdsa_algorithms[curve].point_length;

            return LIBSSH2_ERROR_NONE;
        }
    }

    return LIBSSH2_ERROR_INVAL;
}

/*
 * Create a IEEE P-1363 signature from a point.
 *
 * The IEEE P-1363 format is defined as r || s,
 * where r and s are of the same length.
 */
static int
_libssh2_wincng_p1363signature_from_point(IN const unsigned char *r,
                                          IN size_t r_len,
                                          IN const unsigned char *s,
                                          IN size_t s_len,
                                          IN libssh2_curve_type curve,
                                          OUT PUCHAR *signature,
                                          OUT size_t *signature_length)
{
    const unsigned char *r_trimmed;
    const unsigned char *s_trimmed;
    size_t r_trimmed_len;
    size_t s_trimmed_len;

    /* Validate parameters */
    if(curve >= ARRAY_SIZE(_wincng_ecdsa_algorithms)) {
        return LIBSSH2_ERROR_INVAL;
    }

    *signature = NULL;
    *signature_length = (size_t)
        _wincng_ecdsa_algorithms[curve].point_length * 2;

    /* Trim leading zero, if any */
    r_trimmed = r;
    r_trimmed_len = r_len;
    if(r_len > 0 && r[0] == '\0') {
        r_trimmed++;
        r_trimmed_len--;
    }

    s_trimmed = s;
    s_trimmed_len = s_len;
    if(s_len > 0 && s[0] == '\0') {
        s_trimmed++;
        s_trimmed_len--;
    }

    /* Concatenate into zero-filled buffer and zero-pad if necessary */
    *signature = calloc(1, *signature_length);
    if(!*signature) {
        return LIBSSH2_ERROR_ALLOC;
    }

    memcpy(
        *signature + (*signature_length / 2) - r_trimmed_len,
        r_trimmed,
        r_trimmed_len);
    memcpy(
        *signature + (*signature_length) - s_trimmed_len,
        s_trimmed,
        s_trimmed_len);

    return LIBSSH2_ERROR_NONE;
}

/*
 * Create a CNG public key from an ECC point.
 */
static int
_libssh2_wincng_publickey_from_point(IN _libssh2_wincng_ecc_keytype keytype,
                                     IN _libssh2_ecdsa_point *point,
                                     OUT BCRYPT_KEY_HANDLE *key)
{

    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    PBCRYPT_ECCKEY_BLOB ecc_blob;
    size_t ecc_blob_len;

    /* Validate parameters */
    if(!key) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(point->x_len != point->y_len) {
        return LIBSSH2_ERROR_INVAL;
    }

    *key = NULL;

    /* Initialize a blob to import */
    ecc_blob_len = sizeof(BCRYPT_ECCKEY_BLOB) + point->x_len + point->y_len;
    ecc_blob = malloc(ecc_blob_len);
    if(!ecc_blob) {
        return LIBSSH2_ERROR_ALLOC;
    }

    ecc_blob->cbKey = point->x_len;
    ecc_blob->dwMagic =
        _wincng_ecdsa_algorithms[point->curve].public_import_magic[keytype];

    /** Copy x, y */
    memcpy(
        (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB),
        point->x,
        point->x_len);
    memcpy(
        (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB) + point->x_len,
        point->y,
        point->y_len);

    status = BCryptImportKeyPair(
        keytype == WINCNG_ECC_KEYTYPE_ECDSA
            ? _libssh2_wincng.hAlgECDSA[point->curve]
            : _libssh2_wincng.hAlgECDH[point->curve],
        NULL,
        BCRYPT_ECCPUBLIC_BLOB,
        key,
        (PUCHAR)ecc_blob,
        (ULONG)ecc_blob_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    result = LIBSSH2_ERROR_NONE;

cleanup:
    free(ecc_blob);
    return result;
}

/*
 * Create a CNG private key from an ECC point.
 */
static int
_libssh2_wincng_privatekey_from_point(IN _libssh2_wincng_ecc_keytype keytype,
                                      IN _libssh2_ecdsa_point *q,
                                      IN unsigned char *d,
                                      IN size_t d_len,
                                      OUT BCRYPT_KEY_HANDLE *key)
{
    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    PBCRYPT_ECCKEY_BLOB ecc_blob;
    size_t ecc_blob_len;

    /* Validate parameters */
    if(!key) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(q->x_len != q->y_len) {
        return LIBSSH2_ERROR_INVAL;
    }

    *key = NULL;

    /* Initialize a blob to import */
    ecc_blob_len =
        sizeof(BCRYPT_ECCPRIVATE_BLOB) + q->x_len + q->y_len + d_len;
    ecc_blob = malloc(ecc_blob_len);
    if(!ecc_blob) {
        return LIBSSH2_ERROR_ALLOC;
    }

    ecc_blob->cbKey = q->x_len;
    ecc_blob->dwMagic =
        _wincng_ecdsa_algorithms[q->curve].private_import_magic[keytype];

    /* Copy x, y, d */
    memcpy(
        (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB),
        q->x,
        q->x_len);
    memcpy(
        (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB) + q->x_len,
        q->y,
        q->y_len);
    memcpy(
        (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB) + q->x_len + q->y_len,
        d,
        d_len);

    status = BCryptImportKeyPair(
        keytype == WINCNG_ECC_KEYTYPE_ECDSA
            ? _libssh2_wincng.hAlgECDSA[q->curve]
            : _libssh2_wincng.hAlgECDH[q->curve],
        NULL,
        BCRYPT_ECCPRIVATE_BLOB,
        key,
        (PUCHAR)ecc_blob,
        (ULONG)ecc_blob_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    result = LIBSSH2_ERROR_NONE;

cleanup:
    free(ecc_blob);
    return result;
}

/*
 * Get the uncompressed point encoding for a CNG key.
 */
static int
_libssh2_wincng_uncompressed_point_from_publickey(
    IN LIBSSH2_SESSION *session,
    IN libssh2_curve_type curve,
    IN BCRYPT_KEY_HANDLE key,
    OUT PUCHAR *encoded_point,
    OUT size_t *encoded_point_len)
{
    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    PBCRYPT_ECCKEY_BLOB ecc_blob = NULL;
    ULONG ecc_blob_len;
    PUCHAR point_x;
    PUCHAR point_y;

    /* Validate parameters */
    if(curve >= ARRAY_SIZE(_wincng_ecdsa_algorithms)) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(!encoded_point || !encoded_point_len) {
        return LIBSSH2_ERROR_INVAL;
    }

    *encoded_point = NULL;
    *encoded_point_len = 0;

    /*
     * Export point as BCRYPT_ECCKEY_BLOB, a dynamically-sized structure.
     */
    status = BCryptExportKey(key,
        NULL,
        BCRYPT_ECCPUBLIC_BLOB,
        NULL,
        0,
        &ecc_blob_len,
        0);
    if(BCRYPT_SUCCESS(status) && ecc_blob_len > 0) {
        ecc_blob = LIBSSH2_ALLOC(session, ecc_blob_len);
        if(!ecc_blob) {
            result = LIBSSH2_ERROR_ALLOC;
            goto cleanup;
        }

        status = BCryptExportKey(key,
            NULL,
            BCRYPT_ECCPUBLIC_BLOB,
            (PUCHAR)ecc_blob,
            ecc_blob_len,
            &ecc_blob_len,
            0);
    }

    if(!BCRYPT_SUCCESS(status)) {
        result = _libssh2_error(session,
            LIBSSH2_ERROR_PUBLICKEY_PROTOCOL,
            "Decoding the ECC public key failed");
        goto cleanup;
    }

    point_x = (PUCHAR)ecc_blob + sizeof(BCRYPT_ECCKEY_BLOB);
    point_y = (PUCHAR)ecc_blob + ecc_blob->cbKey + sizeof(BCRYPT_ECCKEY_BLOB);

    /*
     * Create uncompressed point, which needs to look like the following:
     *
     * struct uncompressed_point {
     *     UCHAR tag = 4; // uncompressed
     *     PUCHAR[size] x;
     *     PUCHAR[size] y;
     * }
     */

    *encoded_point_len = (size_t)ecc_blob->cbKey * 2 + 1;
    *encoded_point = LIBSSH2_ALLOC(session, *encoded_point_len);
    if(!*encoded_point) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    **encoded_point = 4;  /* Uncompressed tag */
    memcpy((*encoded_point) + 1, point_x, ecc_blob->cbKey);
    memcpy((*encoded_point) + 1 + ecc_blob->cbKey, point_y, ecc_blob->cbKey);

cleanup:
    if(ecc_blob) {
        LIBSSH2_FREE(session, ecc_blob);
    }

    return result;
}

static void
_libssh_wincng_reverse_bytes(IN PUCHAR buffer,
                             IN size_t buffer_len)
{
    PUCHAR start = buffer;
    PUCHAR end = buffer + buffer_len - 1;
    while(start < end) {
        unsigned char tmp = *end;
        *end = *start;
        *start = tmp;
        start++;
        end--;
    }
}

/*******************************************************************/
/*
 * Windows CNG backend: ECDSA functions
 */

void
_libssh2_wincng_ecdsa_free(IN _libssh2_wincng_ecdsa_key *key)
{
    if(!key) {
        return;
    }

    (void)BCryptDestroyKey(key->handle);
    free(key);
}


/*
 * _libssh2_ecdsa_create_key
 *
 * Creates a local private ECDH key based on input curve
 * and returns the public key in uncompressed point encoding.
 */

int
_libssh2_wincng_ecdh_create_key(IN LIBSSH2_SESSION *session,
                                OUT _libssh2_wincng_ecdsa_key **privatekey,
                                OUT unsigned char **encoded_publickey,
                                OUT size_t *encoded_publickey_len,
                                IN libssh2_curve_type curve)
{
    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    BCRYPT_KEY_HANDLE key_handle = NULL;

    /* Validate parameters */
    if(curve >= ARRAY_SIZE(_wincng_ecdsa_algorithms)) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(!_libssh2_wincng.hAlgECDH[curve]) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(!privatekey || !encoded_publickey || !encoded_publickey_len) {
        return LIBSSH2_ERROR_INVAL;
    }

    *privatekey = NULL;
    *encoded_publickey = NULL;
    *encoded_publickey_len = 0;

    /* Create an ECDH key pair using the requested curve */
    status = BCryptGenerateKeyPair(
        _libssh2_wincng.hAlgECDH[curve],
        &key_handle,
        _wincng_ecdsa_algorithms[curve].key_length,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = _libssh2_error(
            session,
            LIBSSH2_ERROR_PUBLICKEY_PROTOCOL,
            "Creating ECC key pair failed");
        goto cleanup;
    }

    status = BCryptFinalizeKeyPair(key_handle, 0);
    if(!BCRYPT_SUCCESS(status)) {
        result = _libssh2_error(
            session,
            LIBSSH2_ERROR_PUBLICKEY_PROTOCOL,
            "Creating ECDH key pair failed");
        goto cleanup;
    }

    result = _libssh2_wincng_uncompressed_point_from_publickey(
        session,
        curve,
        key_handle,
        encoded_publickey,
        encoded_publickey_len);
    if(result != LIBSSH2_ERROR_NONE) {
        result = _libssh2_error(
            session,
            LIBSSH2_ERROR_PUBLICKEY_PROTOCOL,
            "Exporting ECDH key pair failed");
    }

    *privatekey = malloc(sizeof(_libssh2_wincng_ecdsa_key));
    if(!*privatekey) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    (*privatekey)->curve = curve;
    (*privatekey)->handle = key_handle;

cleanup:
    if(result != LIBSSH2_ERROR_NONE && key_handle) {
        (void)BCryptDestroyKey(key_handle);
    }

    if(result != LIBSSH2_ERROR_NONE && *privatekey) {
        free(*privatekey);
    }

    return result;
}

/*
 * _libssh2_ecdsa_curve_name_with_octal_new
 *
 * Creates an ECDSA public key from an uncompressed point.
 */

int
_libssh2_wincng_ecdsa_curve_name_with_octal_new(
    OUT _libssh2_wincng_ecdsa_key **key,
    IN const unsigned char *publickey_encoded,
    IN size_t publickey_encoded_len,
    IN libssh2_curve_type curve)
{
    int result = LIBSSH2_ERROR_NONE;

    BCRYPT_KEY_HANDLE publickey_handle;
    _libssh2_ecdsa_point publickey;

    /* Validate parameters */
    if(curve >= ARRAY_SIZE(_wincng_ecdsa_algorithms)) {
        return LIBSSH2_ERROR_INVAL;
    }

    if(!key) {
        return LIBSSH2_ERROR_INVAL;
    }

    *key = NULL;

    result = _libssh2_wincng_ecdsa_decode_uncompressed_point(
        publickey_encoded,
        publickey_encoded_len,
        &publickey);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_wincng_publickey_from_point(
        WINCNG_ECC_KEYTYPE_ECDSA,
        &publickey,
        &publickey_handle);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    *key = malloc(sizeof(_libssh2_wincng_ecdsa_key));
    if(!*key) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    (*key)->handle = publickey_handle;
    (*key)->curve = curve;

cleanup:

    return result;
}

/*
 * _libssh2_ecdh_gen_k
 *
 * Computes the shared secret K given a local private key,
 * remote public key and length
 */

int
_libssh2_wincng_ecdh_gen_k(OUT _libssh2_bn **secret,
                           IN _libssh2_wincng_ecdsa_key *privatekey,
                           IN const unsigned char *server_publickey_encoded,
                           IN size_t server_publickey_encoded_len)
{
    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    BCRYPT_KEY_HANDLE publickey_handle;
    BCRYPT_SECRET_HANDLE agreed_secret_handle = NULL;
    ULONG secret_len;
    _libssh2_ecdsa_point server_publickey;

    /* Validate parameters */
    if(!secret) {
        return LIBSSH2_ERROR_INVAL;
    }

    *secret = NULL;

    /* Decode the public key */
    result = _libssh2_wincng_ecdsa_decode_uncompressed_point(
        server_publickey_encoded,
        server_publickey_encoded_len,
        &server_publickey);
    if(result != LIBSSH2_ERROR_NONE) {
        return result;
    }

    result = _libssh2_wincng_publickey_from_point(
        WINCNG_ECC_KEYTYPE_ECDH,
        &server_publickey,
        &publickey_handle);
    if(result != LIBSSH2_ERROR_NONE) {
        return result;
    }

    /* Establish the shared secret between ourselves and the peer */
    status = BCryptSecretAgreement(
        privatekey->handle,
        publickey_handle,
        &agreed_secret_handle,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    /* Compute the size of the buffer that is needed to hold the derived
     * shared secret.
     *
     * NB. The use of BCRYPT_KDF_RAW_SECRET requires Windows 10 or newer.
     * On older versions, the BCryptDeriveKey returns STATUS_NOT_SUPPORTED.
     */
    status = BCryptDeriveKey(
        agreed_secret_handle,
        BCRYPT_KDF_RAW_SECRET,
        NULL,
        NULL,
        0,
        &secret_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    /* Allocate a secret bignum to be ready to receive the derived secret */
    *secret = _libssh2_wincng_bignum_init();
    if(!*secret) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    if(_libssh2_wincng_bignum_resize(*secret, secret_len)) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    /* And populate the secret bignum */
    status = BCryptDeriveKey(
        agreed_secret_handle,
        BCRYPT_KDF_RAW_SECRET,
        NULL,
        (*secret)->bignum,
        secret_len,
        &secret_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    /* BCRYPT_KDF_RAW_SECRET returns the little-endian representation of the
     * raw secret, so we need to swap it to big endian order.
     */

    _libssh_wincng_reverse_bytes((*secret)->bignum, secret_len);

    result = LIBSSH2_ERROR_NONE;

cleanup:
    if(result != LIBSSH2_ERROR_NONE && agreed_secret_handle) {
        _libssh2_wincng_bignum_free(*secret);
    }

    if(result != LIBSSH2_ERROR_NONE && agreed_secret_handle) {
        BCryptDestroySecret(agreed_secret_handle);
    }

    return result;
}

/*
 * _libssh2_ecdsa_curve_type_from_name
 *
 */
int
_libssh2_wincng_ecdsa_curve_type_from_name(IN const char *name,
                                           OUT libssh2_curve_type *out_curve)
{
    unsigned int curve;

    /* Validate parameters */
    if(!out_curve) {
        return LIBSSH2_ERROR_INVAL;
    }

    for(curve = 0; curve < ARRAY_SIZE(_wincng_ecdsa_algorithms); curve++) {
        if(strcmp(name, _wincng_ecdsa_algorithms[curve].name) == 0) {
            *out_curve = curve;
            return LIBSSH2_ERROR_NONE;
        }
    }

    return LIBSSH2_ERROR_INVAL;
}

/*
 * _libssh2_ecdsa_verify
 *
 * Verifies the ECDSA signature of a hashed message
 *
 */

int
_libssh2_wincng_ecdsa_verify(IN _libssh2_wincng_ecdsa_key *key,
                             IN const unsigned char *r,
                             IN size_t r_len,
                             IN const unsigned char *s,
                             IN size_t s_len,
                             IN const unsigned char *m,
                             IN size_t m_len)
{
    int result = LIBSSH2_ERROR_NONE;
    NTSTATUS status;

    PUCHAR signature_p1363 = NULL;
    size_t signature_p1363_len;
    ULONG hash_len;
    PUCHAR hash = NULL;
    BCRYPT_ALG_HANDLE hash_alg;

    /* CNG expects signatures in IEEE P-1363 format. */
    result = _libssh2_wincng_p1363signature_from_point(
        r,
        r_len,
        s,
        s_len,
        _libssh2_wincng_ecdsa_get_curve_type(key),
        &signature_p1363,
        &signature_p1363_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Create hash over m */
    switch(_libssh2_wincng_ecdsa_get_curve_type(key)) {
    case LIBSSH2_EC_CURVE_NISTP256:
        hash_len = 256/8;
        hash_alg = _libssh2_wincng.hAlgHashSHA256;
        break;

    case LIBSSH2_EC_CURVE_NISTP384:
        hash_len = 384/8;
        hash_alg = _libssh2_wincng.hAlgHashSHA384;
        break;

    case LIBSSH2_EC_CURVE_NISTP521:
        hash_len = 512/8;
        hash_alg = _libssh2_wincng.hAlgHashSHA512;
        break;

    default:
        return LIBSSH2_ERROR_INVAL;
    }

    hash = malloc(hash_len);
    result = _libssh2_wincng_hash(
        m,
        (ULONG)m_len,
        hash_alg,
        hash,
        hash_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Verify signature over hash */
    status = BCryptVerifySignature(
        key->handle,
        NULL,
        hash,
        hash_len,
        signature_p1363,
        (ULONG)signature_p1363_len,
        0);

    if(status == STATUS_INVALID_SIGNATURE) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }
    else if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    result = LIBSSH2_ERROR_NONE;

cleanup:
    if(hash) {
        free(hash);
    }

    if(signature_p1363) {
        free(signature_p1363);
    }

    return result;
}

/*
 *_libssh2_ecdsa_new_private
 *
 * Creates a new private key given a file path and password
 *
 */

int
_libssh2_wincng_ecdsa_new_private(OUT _libssh2_wincng_ecdsa_key **key,
                                  IN LIBSSH2_SESSION *session,
                                  IN const char *filename,
                                  IN const unsigned char *passphrase)
{
    int result;

    FILE *file_handle = NULL;
    unsigned char *data = NULL;
    size_t datalen = 0;

    /* Validate parameters */
    if(!key || !session || !filename) {
        return LIBSSH2_ERROR_INVAL;
    }

    *key = NULL;

    if(passphrase && strlen((const char *)passphrase) > 0) {
        return _libssh2_error(
            session,
            LIBSSH2_ERROR_INVAL,
            "Passphrase-protected ECDSA private key files are unsupported");
    }

    file_handle = fopen(filename, FOPEN_READTEXT);
    if(!file_handle) {
        result = _libssh2_error(
            session,
            LIBSSH2_ERROR_INVAL,
            "Opening the private key file failed");
        goto cleanup;
    }

    result = _libssh2_pem_parse(session,
        PEM_ECDSA_HEADER,
        PEM_ECDSA_FOOTER,
        passphrase,
        file_handle,
        &data,
        &datalen);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_wincng_ecdsa_new_private_frommemory(
        key,
        session,
        (const char *)data,
        datalen,
        passphrase);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

cleanup:
    if(file_handle) {
        fclose(file_handle);
    }

    if(data) {
        LIBSSH2_FREE(session, data);
    }

    return result;
}

int
_libssh2_wincng_parse_ecdsa_privatekey(OUT _libssh2_wincng_ecdsa_key **key,
                                       IN unsigned char *privatekey,
                                       IN size_t privatekey_len)
{
    char *keytype = NULL;
    size_t keytype_len;

    unsigned char *ignore;
    size_t ignore_len;

    unsigned char *publickey;
    size_t publickey_len;

    libssh2_curve_type curve_type;
    int result;
    uint32_t check1, check2;
    struct string_buf data_buffer;

    _libssh2_ecdsa_point q;
    unsigned char *d;
    size_t d_len;

    BCRYPT_KEY_HANDLE key_handle = NULL;

    *key = NULL;

    data_buffer.data = privatekey;
    data_buffer.dataptr = privatekey;
    data_buffer.len = privatekey_len;

    /* Read the 2 checkints and check that they match */
    result = _libssh2_get_u32(&data_buffer, &check1);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_get_u32(&data_buffer, &check2);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    if(check1 != check2) {
        result = LIBSSH2_ERROR_FILE;
        goto cleanup;
    }

    /* What follows is a key as defined in */
    /* draft-miller-ssh-agent, section-3.2.2 */

    /* Read the key type */
    result = _libssh2_get_string(&data_buffer,
                                 (unsigned char **)&keytype,
                                 &keytype_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_wincng_ecdsa_curve_type_from_name(keytype, &curve_type);
    if(result < 0) {
        goto cleanup;
    }

    /* Read the curve */
    result = _libssh2_get_string(&data_buffer, &ignore, &ignore_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Read Q */
    result = _libssh2_get_string(&data_buffer, &publickey, &publickey_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_wincng_ecdsa_decode_uncompressed_point(
        publickey,
        publickey_len,
        &q);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Read d */
    result = _libssh2_get_bignum_bytes(&data_buffer, &d, &d_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Ignore the rest (comment, etc) */

    /* Use Q and d to create a key handle */
    result = _libssh2_wincng_privatekey_from_point(
        WINCNG_ECC_KEYTYPE_ECDSA,
        &q,
        d,
        d_len,
        &key_handle);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    *key = malloc(sizeof(_libssh2_wincng_ecdsa_key));
    if(!*key) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    (*key)->curve = q.curve;
    (*key)->handle = key_handle;

    result = LIBSSH2_ERROR_NONE;

cleanup:
    if(result != LIBSSH2_ERROR_NONE && key_handle) {
        (void)BCryptDestroyKey(key_handle);
    }

    return result;
}

/*
 * _libssh2_ecdsa_new_private
 *
 * Creates a new private key given a file data and password.
 * ECDSA private key files use the decoding defined in PROTOCOL.key
 * in the OpenSSL source tree.
 */
int
_libssh2_wincng_ecdsa_new_private_frommemory(
    OUT _libssh2_wincng_ecdsa_key **key,
    IN LIBSSH2_SESSION *session,
    IN const char *data,
    IN size_t data_len,
    IN const unsigned char *passphrase)
{
    int result;

    struct string_buf data_buffer;
    uint32_t index;
    uint32_t key_count;
    unsigned char *privatekey;
    size_t privatekey_len;

    /* Validate parameters */
    if(!key || !session || !data) {
        return LIBSSH2_ERROR_INVAL;
    }

    *key = NULL;

    if(passphrase && strlen((const char *)passphrase) > 0) {
        return _libssh2_error(
            session,
            LIBSSH2_ERROR_INVAL,
            "Passphrase-protected ECDSA private key files are unsupported");
    }

    /* Read OPENSSL_PRIVATEKEY_AUTH_MAGIC */
    if(strncmp(data, OPENSSL_PRIVATEKEY_AUTH_MAGIC, data_len) != 0) {
        result = -1;
        goto cleanup;
    }

    data_buffer.len = data_len;
    data_buffer.data = (unsigned char *)data;
    data_buffer.dataptr =
        (unsigned char *)data + strlen(OPENSSL_PRIVATEKEY_AUTH_MAGIC) + 1;

    /* Read ciphername, should be 'none' as we don't support passphrases */
    result = _libssh2_match_string(&data_buffer, "none");
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Read kdfname, should be 'none' as we don't support passphrases */
    result = _libssh2_match_string(&data_buffer, "none");
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Read kdfoptions, should be empty */
    result = _libssh2_match_string(&data_buffer, "");
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    /* Read number of keys N */
    result = _libssh2_get_u32(&data_buffer, &key_count);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    if(key_count == 0) {
        result = LIBSSH2_ERROR_FILE;
        goto cleanup;
    }

    /* Skip all public keys */
    for(index = 0; index < key_count; index++) {
        unsigned char *publickey;
        size_t publickey_len;

        result = _libssh2_get_string(&data_buffer, &publickey, &publickey_len);
        if(result != LIBSSH2_ERROR_NONE) {
            goto cleanup;
        }
    }

    /* Read first private key */
    result = _libssh2_get_string(&data_buffer, &privatekey, &privatekey_len);
    if(result != LIBSSH2_ERROR_NONE) {
        goto cleanup;
    }

    result = _libssh2_wincng_parse_ecdsa_privatekey(
        key,
        privatekey,
        privatekey_len);

cleanup:
    if(result != LIBSSH2_ERROR_NONE) {
        return _libssh2_error(
            session,
            result,
            "The key is malformed");
    }

    return result;
}

/*
 * _libssh2_ecdsa_sign
 *
 * Computes the ECDSA signature of a previously-hashed message
 *
 */

int
_libssh2_wincng_ecdsa_sign(IN LIBSSH2_SESSION *session,
                           IN _libssh2_wincng_ecdsa_key *key,
                           IN const unsigned char *hash,
                           IN size_t hash_len,
                           OUT unsigned char **signature,
                           OUT size_t *signature_len)
{
    NTSTATUS status;
    int result = LIBSSH2_ERROR_NONE;

    unsigned char *hash_buffer;

    unsigned char *cng_signature = NULL;
    ULONG cng_signature_len;

    ULONG signature_maxlen;
    unsigned char *signature_ptr;

    *signature = NULL;
    *signature_len = 0;

    /* CNG expects a mutable buffer */
    hash_buffer = malloc(hash_len);
    if(!hash_buffer) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    memcpy(hash_buffer, hash, hash_len);

    status = BCryptSignHash(
        key->handle,
        NULL,
        hash_buffer,
        (ULONG)hash_len,
        NULL,
        0,
        &cng_signature_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    cng_signature = malloc(cng_signature_len);
    if(!cng_signature) {
        result = LIBSSH2_ERROR_ALLOC;
        goto cleanup;
    }

    status = BCryptSignHash(
        key->handle,
        NULL,
        hash_buffer,
        (ULONG)hash_len,
        cng_signature,
        cng_signature_len,
        &cng_signature_len,
        0);
    if(!BCRYPT_SUCCESS(status)) {
        result = LIBSSH2_ERROR_PUBLICKEY_PROTOCOL;
        goto cleanup;
    }

    /*
        cng_signature is in IEEE P-1163 format: r || s.
        Convert to ecdsa_signature_blob: mpint(r) || mpint(s)
    */

    signature_maxlen =
        cng_signature_len / 2 + 5 + /* mpint(r) */
        cng_signature_len / 2 + 5;  /* mpint(s) */

    *signature = LIBSSH2_ALLOC(session, signature_maxlen);
    signature_ptr = *signature;

    _libssh2_store_bignum2_bytes(
        &signature_ptr,
        cng_signature,
        cng_signature_len / 2);

    _libssh2_store_bignum2_bytes(
        &signature_ptr,
        cng_signature + (cng_signature_len / 2),
        cng_signature_len / 2);

    *signature_len = signature_ptr - *signature;

cleanup:
    if(cng_signature) {
        free(cng_signature);
    }

    if(hash_buffer) {
        free(hash_buffer);
    }

    return result;
}

/*
 * _libssh2_ecdsa_get_curve_type
 *
 * returns key curve type that maps to libssh2_curve_type
 *
 */

libssh2_curve_type
_libssh2_wincng_ecdsa_get_curve_type(IN _libssh2_wincng_ecdsa_key *key)
{
    return key->curve;
}

#endif

/*******************************************************************/
/*
 * Windows CNG backend: Key functions
 */

#ifdef HAVE_LIBCRYPT32
static DWORD
_libssh2_wincng_pub_priv_write(unsigned char *key,
                               DWORD offset,
                               const unsigned char *bignum,
                               const DWORD length)
{
    _libssh2_htonu32(key + offset, length);
    offset += 4;

    memcpy(key + offset, bignum, length);
    offset += length;

    return offset;
}

static int
_libssh2_wincng_pub_priv_keyfile_parse(LIBSSH2_SESSION *session,
                                       unsigned char **method,
                                       size_t *method_len,
                                       unsigned char **pubkeydata,
                                       size_t *pubkeydata_len,
                                       unsigned char *pbEncoded,
                                       size_t cbEncoded)
{
    unsigned char **rpbDecoded = NULL;
    DWORD *rcbDecoded = NULL;
    unsigned char *key = NULL, *mth = NULL;
    DWORD keylen = 0, mthlen = 0;
    DWORD index, offset, length = 0;
    int ret;

    ret = _libssh2_wincng_asn_decode_bns(pbEncoded, (DWORD)cbEncoded,
                                         &rpbDecoded, &rcbDecoded, &length);

    _libssh2_wincng_safe_free(pbEncoded, cbEncoded);

    if(ret) {
        return -1;
    }


    if(length == 9) { /* private RSA key */
        mthlen = 7;
        mth = LIBSSH2_ALLOC(session, mthlen);
        if(mth) {
            memcpy(mth, "ssh-rsa", mthlen);
        }
        else {
            ret = -1;
        }


        keylen = 4 + mthlen + 4 + rcbDecoded[2] + 4 + rcbDecoded[1];
        key = LIBSSH2_ALLOC(session, keylen);
        if(key) {
            offset = _libssh2_wincng_pub_priv_write(key, 0, mth, mthlen);

            offset = _libssh2_wincng_pub_priv_write(key, offset,
                                                    rpbDecoded[2],
                                                    rcbDecoded[2]);

            _libssh2_wincng_pub_priv_write(key, offset,
                                           rpbDecoded[1],
                                           rcbDecoded[1]);
        }
        else {
            ret = -1;
        }

    }
    else if(length == 6) { /* private DSA key */
        mthlen = 7;
        mth = LIBSSH2_ALLOC(session, mthlen);
        if(mth) {
            memcpy(mth, "ssh-dss", mthlen);
        }
        else {
            ret = -1;
        }

        keylen = 4 + mthlen + 4 + rcbDecoded[1] + 4 + rcbDecoded[2]
                            + 4 + rcbDecoded[3] + 4 + rcbDecoded[4];
        key = LIBSSH2_ALLOC(session, keylen);
        if(key) {
            offset = _libssh2_wincng_pub_priv_write(key, 0, mth, mthlen);

            offset = _libssh2_wincng_pub_priv_write(key, offset,
                                                    rpbDecoded[1],
                                                    rcbDecoded[1]);

            offset = _libssh2_wincng_pub_priv_write(key, offset,
                                                    rpbDecoded[2],
                                                    rcbDecoded[2]);

            offset = _libssh2_wincng_pub_priv_write(key, offset,
                                                    rpbDecoded[3],
                                                    rcbDecoded[3]);

            _libssh2_wincng_pub_priv_write(key, offset,
                                           rpbDecoded[4],
                                           rcbDecoded[4]);
        }
        else {
            ret = -1;
        }

    }
    else {
        ret = -1;
    }


    for(index = 0; index < length; index++) {
        _libssh2_wincng_safe_free(rpbDecoded[index], rcbDecoded[index]);
        rpbDecoded[index] = NULL;
        rcbDecoded[index] = 0;
    }

    free(rpbDecoded);
    free(rcbDecoded);


    if(ret) {
        if(mth)
            LIBSSH2_FREE(session, mth);
        if(key)
            LIBSSH2_FREE(session, key);
    }
    else {
        *method = mth;
        *method_len = mthlen;
        *pubkeydata = key;
        *pubkeydata_len = keylen;
    }

    return ret;
}
#endif /* HAVE_LIBCRYPT32 */

int
_libssh2_wincng_pub_priv_keyfile(LIBSSH2_SESSION *session,
                                 unsigned char **method,
                                 size_t *method_len,
                                 unsigned char **pubkeydata,
                                 size_t *pubkeydata_len,
                                 const char *privatekey,
                                 const char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    ret = _libssh2_wincng_load_private(session, privatekey,
                                       (const unsigned char *)passphrase,
                                       &pbEncoded, &cbEncoded, 1, 1);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_pub_priv_keyfile_parse(session, method, method_len,
                                                  pubkeydata, pubkeydata_len,
                                                  pbEncoded, cbEncoded);
#else
    (void)method;
    (void)method_len;
    (void)pubkeydata;
    (void)pubkeydata_len;
    (void)privatekey;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                          "Unable to load public key from private key file: "
                          "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

int
_libssh2_wincng_pub_priv_keyfilememory(LIBSSH2_SESSION *session,
                                       unsigned char **method,
                                       size_t *method_len,
                                       unsigned char **pubkeydata,
                                       size_t *pubkeydata_len,
                                       const char *privatekeydata,
                                       size_t privatekeydata_len,
                                       const char *passphrase)
{
#ifdef HAVE_LIBCRYPT32
    unsigned char *pbEncoded;
    size_t cbEncoded;
    int ret;

    ret = _libssh2_wincng_load_private_memory(session, privatekeydata,
                                              privatekeydata_len,
                                              (const unsigned char *)
                                                  passphrase,
                                              &pbEncoded, &cbEncoded, 1, 1);
    if(ret) {
        return -1;
    }

    return _libssh2_wincng_pub_priv_keyfile_parse(session, method, method_len,
                                                  pubkeydata, pubkeydata_len,
                                                  pbEncoded, cbEncoded);
#else
    (void)method;
    (void)method_len;
    (void)pubkeydata_len;
    (void)pubkeydata;
    (void)privatekeydata;
    (void)privatekeydata_len;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_METHOD_NOT_SUPPORTED,
                    "Unable to extract public key from private key in memory: "
                    "Method unsupported in Windows CNG backend");
#endif /* HAVE_LIBCRYPT32 */
}

int
_libssh2_wincng_sk_pub_keyfilememory(LIBSSH2_SESSION *session,
                                     unsigned char **method,
                                     size_t *method_len,
                                     unsigned char **pubkeydata,
                                     size_t *pubkeydata_len,
                                     int *algorithm,
                                     unsigned char *flags,
                                     const char **application,
                                     const unsigned char **key_handle,
                                     size_t *handle_len,
                                     const char *privatekeydata,
                                     size_t privatekeydata_len,
                                     const char *passphrase)
{
    (void)method;
    (void)method_len;
    (void)pubkeydata;
    (void)pubkeydata_len;
    (void)algorithm;
    (void)flags;
    (void)application;
    (void)key_handle;
    (void)handle_len;
    (void)privatekeydata;
    (void)privatekeydata_len;
    (void)passphrase;

    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                    "Unable to extract public SK key from private key file: "
                    "Method unimplemented in Windows CNG backend");
}

/*******************************************************************/
/*
 * Windows CNG backend: Cipher functions
 */

int
_libssh2_wincng_cipher_init(_libssh2_cipher_ctx *ctx,
                            _libssh2_cipher_type(type),
                            unsigned char *iv,
                            unsigned char *secret,
                            int encrypt)
{
    BCRYPT_KEY_HANDLE hKey;
    BCRYPT_KEY_DATA_BLOB_HEADER *header;
    unsigned char *pbKeyObject, *pbIV, *pbCtr, *pbIVCopy;
    ULONG dwKeyObject, dwIV, dwCtrLength, dwBlockLength, cbData, keylen;
    int ret;

    (void)encrypt;

    ret = BCryptGetProperty(*type.phAlg, BCRYPT_OBJECT_LENGTH,
                            (unsigned char *)&dwKeyObject,
                            sizeof(dwKeyObject),
                            &cbData, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        return -1;
    }

    ret = BCryptGetProperty(*type.phAlg, BCRYPT_BLOCK_LENGTH,
                            (unsigned char *)&dwBlockLength,
                            sizeof(dwBlockLength),
                            &cbData, 0);
    if(!BCRYPT_SUCCESS(ret)) {
        return -1;
    }

    pbKeyObject = malloc(dwKeyObject);
    if(!pbKeyObject) {
        return -1;
    }


    keylen = (ULONG)sizeof(BCRYPT_KEY_DATA_BLOB_HEADER) +
             type.dwKeyLength;
    header = (BCRYPT_KEY_DATA_BLOB_HEADER *)malloc(keylen);
    if(!header) {
        free(pbKeyObject);
        return -1;
    }


    header->dwMagic = BCRYPT_KEY_DATA_BLOB_MAGIC;
    header->dwVersion = BCRYPT_KEY_DATA_BLOB_VERSION1;
    header->cbKeyData = type.dwKeyLength;

    memcpy((unsigned char *)header + sizeof(BCRYPT_KEY_DATA_BLOB_HEADER),
           secret, type.dwKeyLength);

    ret = BCryptImportKey(*type.phAlg, NULL, BCRYPT_KEY_DATA_BLOB, &hKey,
                          pbKeyObject, dwKeyObject,
                          (PUCHAR)header, keylen, 0);

    _libssh2_wincng_safe_free(header, keylen);

    if(!BCRYPT_SUCCESS(ret)) {
        _libssh2_wincng_safe_free(pbKeyObject, dwKeyObject);
        return -1;
    }

    pbIV = NULL;
    pbCtr = NULL;
    dwIV = 0;
    dwCtrLength = 0;

    if(type.useIV || type.ctrMode) {
        pbIVCopy = malloc(dwBlockLength);
        if(!pbIVCopy) {
            BCryptDestroyKey(hKey);
            _libssh2_wincng_safe_free(pbKeyObject, dwKeyObject);
            return -1;
        }
        memcpy(pbIVCopy, iv, dwBlockLength);

        if(type.ctrMode) {
            pbCtr = pbIVCopy;
            dwCtrLength = dwBlockLength;
        }
        else if(type.useIV) {
            pbIV = pbIVCopy;
            dwIV = dwBlockLength;
        }
    }

    ctx->hKey = hKey;
    ctx->pbKeyObject = pbKeyObject;
    ctx->pbIV = pbIV;
    ctx->pbCtr = pbCtr;
    ctx->dwKeyObject = dwKeyObject;
    ctx->dwIV = dwIV;
    ctx->dwBlockLength = dwBlockLength;
    ctx->dwCtrLength = dwCtrLength;

    return 0;
}

int
_libssh2_wincng_cipher_crypt(_libssh2_cipher_ctx *ctx,
                             _libssh2_cipher_type(type),
                             int encrypt,
                             unsigned char *block,
                             size_t blocklen, int firstlast)
{
    unsigned char *pbOutput, *pbInput;
    ULONG cbOutput, cbInput;
    NTSTATUS ret;

    (void)type;
    (void)firstlast;

    cbInput = (ULONG)blocklen;

    if(type.ctrMode) {
        pbInput = ctx->pbCtr;
    }
    else {
        pbInput = block;
    }

    if(encrypt || type.ctrMode) {
        ret = BCryptEncrypt(ctx->hKey, pbInput, cbInput, NULL,
                            ctx->pbIV, ctx->dwIV, NULL, 0, &cbOutput, 0);
    }
    else {
        ret = BCryptDecrypt(ctx->hKey, pbInput, cbInput, NULL,
                            ctx->pbIV, ctx->dwIV, NULL, 0, &cbOutput, 0);
    }
    if(BCRYPT_SUCCESS(ret)) {
        pbOutput = malloc(cbOutput);
        if(pbOutput) {
            if(encrypt || type.ctrMode) {
                ret = BCryptEncrypt(ctx->hKey, pbInput, cbInput, NULL,
                                    ctx->pbIV, ctx->dwIV,
                                    pbOutput, cbOutput, &cbOutput, 0);
            }
            else {
                ret = BCryptDecrypt(ctx->hKey, pbInput, cbInput, NULL,
                                    ctx->pbIV, ctx->dwIV,
                                    pbOutput, cbOutput, &cbOutput, 0);
            }
            if(BCRYPT_SUCCESS(ret)) {
                if(type.ctrMode) {
                    _libssh2_xor_data(block, block, pbOutput, blocklen);
                    _libssh2_aes_ctr_increment(ctx->pbCtr, ctx->dwCtrLength);
                }
                else {
                    memcpy(block, pbOutput, cbOutput);
                }
            }

            _libssh2_wincng_safe_free(pbOutput, cbOutput);
        }
        else
            ret = (NTSTATUS)STATUS_NO_MEMORY;
    }

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

void
_libssh2_wincng_cipher_dtor(_libssh2_cipher_ctx *ctx)
{
    BCryptDestroyKey(ctx->hKey);
    ctx->hKey = NULL;

    _libssh2_wincng_safe_free(ctx->pbKeyObject, ctx->dwKeyObject);
    ctx->pbKeyObject = NULL;
    ctx->dwKeyObject = 0;

    _libssh2_wincng_safe_free(ctx->pbIV, ctx->dwBlockLength);
    ctx->pbIV = NULL;
    ctx->dwBlockLength = 0;

    _libssh2_wincng_safe_free(ctx->pbCtr, ctx->dwCtrLength);
    ctx->pbCtr = NULL;
    ctx->dwCtrLength = 0;
}


/*******************************************************************/
/*
 * Windows CNG backend: BigNumber functions
 */

_libssh2_bn *
_libssh2_wincng_bignum_init(void)
{
    _libssh2_bn *bignum;

    bignum = (_libssh2_bn *)malloc(sizeof(_libssh2_bn));
    if(bignum) {
        bignum->bignum = NULL;
        bignum->length = 0;
    }

    return bignum;
}

static int
_libssh2_wincng_bignum_resize(_libssh2_bn *bn, ULONG length)
{
    unsigned char *bignum;

    if(!bn)
        return -1;

    if(length == bn->length)
        return 0;

    if(bn->bignum && bn->length > 0 && length < bn->length) {
        _libssh2_explicit_zero(bn->bignum + length, bn->length - length);
    }

    bignum = realloc(bn->bignum, length);
    if(!bignum)
        return -1;

    bn->bignum = bignum;
    bn->length = length;

    return 0;
}

static int
_libssh2_wincng_bignum_rand(_libssh2_bn *rnd, int bits, int top, int bottom)
{
    unsigned char *bignum;
    ULONG length;

    if(!rnd)
        return -1;

    length = (ULONG) (ceil(((double)bits) / 8.0) * sizeof(unsigned char));
    if(_libssh2_wincng_bignum_resize(rnd, length))
        return -1;

    bignum = rnd->bignum;

    if(_libssh2_wincng_random(bignum, length))
        return -1;

    /* calculate significant bits in most significant byte */
    bits %= 8;
    if(bits == 0)
        bits = 8;

    /* fill most significant byte with zero padding */
    bignum[0] &= (unsigned char)((1 << bits) - 1);

    /* set most significant bits in most significant byte */
    if(top == 0)
        bignum[0] |= (unsigned char)(1 << (bits - 1));
    else if(top == 1)
        bignum[0] |= (unsigned char)(3 << (bits - 2));

    /* make odd by setting first bit in least significant byte */
    if(bottom)
        bignum[length - 1] |= 1;

    return 0;
}

static int
_libssh2_wincng_bignum_mod_exp(_libssh2_bn *r,
                               _libssh2_bn *a,
                               _libssh2_bn *p,
                               _libssh2_bn *m)
{
    BCRYPT_KEY_HANDLE hKey;
    BCRYPT_RSAKEY_BLOB *rsakey;
    unsigned char *bignum;
    ULONG keylen, offset, length;
    NTSTATUS ret;

    if(!r || !a || !p || !m)
        return -1;

    offset = sizeof(BCRYPT_RSAKEY_BLOB);
    keylen = offset + p->length + m->length;

    rsakey = (BCRYPT_RSAKEY_BLOB *)malloc(keylen);
    if(!rsakey)
        return -1;


    /* https://msdn.microsoft.com/library/windows/desktop/aa375531.aspx */
    rsakey->Magic = BCRYPT_RSAPUBLIC_MAGIC;
    rsakey->BitLength = m->length * 8;
    rsakey->cbPublicExp = p->length;
    rsakey->cbModulus = m->length;
    rsakey->cbPrime1 = 0;
    rsakey->cbPrime2 = 0;

    memcpy((unsigned char *)rsakey + offset, p->bignum, p->length);
    offset += p->length;

    memcpy((unsigned char *)rsakey + offset, m->bignum, m->length);
    offset = 0;

    ret = BCryptImportKeyPair(_libssh2_wincng.hAlgRSA, NULL,
                              BCRYPT_RSAPUBLIC_BLOB, &hKey,
                              (PUCHAR)rsakey, keylen, 0);
    if(BCRYPT_SUCCESS(ret)) {
        ret = BCryptEncrypt(hKey, a->bignum, a->length, NULL, NULL, 0,
                            NULL, 0, &length, BCRYPT_PAD_NONE);
        if(BCRYPT_SUCCESS(ret)) {
            if(!_libssh2_wincng_bignum_resize(r, length)) {
                length = max(a->length, length);
                bignum = malloc(length);
                if(bignum) {
                    memcpy_with_be_padding(bignum, length,
                                           a->bignum, a->length);

                    ret = BCryptEncrypt(hKey, bignum, length, NULL, NULL, 0,
                                        r->bignum, r->length, &offset,
                                        BCRYPT_PAD_NONE);

                    _libssh2_wincng_safe_free(bignum, length);

                    if(BCRYPT_SUCCESS(ret)) {
                        _libssh2_wincng_bignum_resize(r, offset);
                    }
                }
                else
                    ret = (NTSTATUS)STATUS_NO_MEMORY;
            }
            else
                ret = (NTSTATUS)STATUS_NO_MEMORY;
        }

        BCryptDestroyKey(hKey);
    }

    _libssh2_wincng_safe_free(rsakey, keylen);

    return BCRYPT_SUCCESS(ret) ? 0 : -1;
}

int
_libssh2_wincng_bignum_set_word(_libssh2_bn *bn, ULONG word)
{
    ULONG offset, number, bits, length;

    if(!bn)
        return -1;

    bits = 0;
    number = word;
    while(number >>= 1)
        bits++;
    bits++;

    length = (ULONG) (ceil(((double)bits) / 8.0) * sizeof(unsigned char));
    if(_libssh2_wincng_bignum_resize(bn, length))
        return -1;

    for(offset = 0; offset < length; offset++)
        bn->bignum[offset] = (word >> (offset * 8)) & 0xff;

    return 0;
}

ULONG
_libssh2_wincng_bignum_bits(const _libssh2_bn *bn)
{
    unsigned char number;
    ULONG offset, length, bits;

    if(!bn || !bn->bignum || !bn->length)
        return 0;

    offset = 0;
    length = bn->length - 1;
    while(!bn->bignum[offset] && offset < length)
        offset++;

    bits = (length - offset) * 8;
    number = bn->bignum[offset];
    while(number >>= 1)
        bits++;
    bits++;

    return bits;
}

int
_libssh2_wincng_bignum_from_bin(_libssh2_bn *bn, ULONG len,
                                const unsigned char *bin)
{
    unsigned char *bignum;
    ULONG offset, length, bits;

    if(!bn || !bin || !len)
        return -1;

    if(_libssh2_wincng_bignum_resize(bn, len))
        return -1;

    memcpy(bn->bignum, bin, len);

    bits = _libssh2_wincng_bignum_bits(bn);
    length = (ULONG) (ceil(((double)bits) / 8.0) * sizeof(unsigned char));

    offset = bn->length - length;
    if(offset > 0) {
        memmove(bn->bignum, bn->bignum + offset, length);

        _libssh2_explicit_zero(bn->bignum + length, offset);

        bignum = realloc(bn->bignum, length);
        if(bignum) {
            bn->bignum = bignum;
            bn->length = length;
        }
        else {
            return -1;
        }
    }

    return 0;
}

int
_libssh2_wincng_bignum_to_bin(const _libssh2_bn *bn, unsigned char *bin)
{
    if(bin && bn && bn->bignum && bn->length > 0) {
        memcpy(bin, bn->bignum, bn->length);
        return 0;
    }

    return -1;
}

void
_libssh2_wincng_bignum_free(_libssh2_bn *bn)
{
    if(bn) {
        if(bn->bignum) {
            _libssh2_wincng_safe_free(bn->bignum, bn->length);
            bn->bignum = NULL;
        }
        bn->length = 0;
        _libssh2_wincng_safe_free(bn, sizeof(_libssh2_bn));
    }
}


/*******************************************************************/
/*
 * Windows CNG backend: Diffie-Hellman support.
 */

void
_libssh2_dh_init(_libssh2_dh_ctx *dhctx)
{
    /* Random from client */
    dhctx->dh_handle = NULL;
    dhctx->dh_params = NULL;
    dhctx->dh_privbn = NULL;
}

void
_libssh2_dh_dtor(_libssh2_dh_ctx *dhctx)
{
    if(dhctx->dh_handle) {
        BCryptDestroyKey(dhctx->dh_handle);
        dhctx->dh_handle = NULL;
    }
    if(dhctx->dh_params) {
        /* Since public dh_params are shared in clear text,
         * we don't need to securely zero them out here */
        free(dhctx->dh_params);
        dhctx->dh_params = NULL;
    }
    if(dhctx->dh_privbn) {
        _libssh2_wincng_bignum_free(dhctx->dh_privbn);
        dhctx->dh_privbn = NULL;
    }
}

static int
round_down(int number, int multiple)
{
    return (number / multiple) * multiple;
}

/* Generates a Diffie-Hellman key pair using base `g', prime `p' and the given
 * `group_order'. Can use the given big number context `bnctx' if needed.  The
 * private key is stored as opaque in the Diffie-Hellman context `*dhctx' and
 * the public key is returned in `public'.  0 is returned upon success, else
 * -1.  */
int
_libssh2_dh_key_pair(_libssh2_dh_ctx *dhctx, _libssh2_bn *public,
                     _libssh2_bn *g, _libssh2_bn *p, int group_order)
{
    const int hasAlgDHwithKDF = _libssh2_wincng.hasAlgDHwithKDF;

    if(group_order < 0)
        return -1;

    while(_libssh2_wincng.hAlgDH && hasAlgDHwithKDF != -1) {
        BCRYPT_DH_PARAMETER_HEADER *dh_params;
        ULONG dh_params_len;
        int status;
        /* Note that the DH provider requires that keys be multiples of 64 bits
         * in length. At the time of writing a practical observed group_order
         * value is 257, so we need to round down to 8 bytes of length (64/8)
         * in order for kex to succeed */
        ULONG key_length_bytes = max((ULONG)round_down(group_order, 8),
                                     max(g->length, p->length));
        BCRYPT_DH_KEY_BLOB *dh_key_blob;
        LPCWSTR key_type;

        /* Prepare a key pair; pass the in the bit length of the key,
         * but the key is not ready for consumption until it is finalized. */
        status = BCryptGenerateKeyPair(_libssh2_wincng.hAlgDH,
                                       &dhctx->dh_handle,
                                       key_length_bytes * 8, 0);
        if(!BCRYPT_SUCCESS(status)) {
            return -1;
        }

        dh_params_len = (ULONG)sizeof(*dh_params) +
                        2 * key_length_bytes;
        dh_params = (BCRYPT_DH_PARAMETER_HEADER *)malloc(dh_params_len);
        if(!dh_params) {
            return -1;
        }

        /* Populate DH parameters blob; after the header follows the `p`
         * value and the `g` value. */
        dh_params->cbLength = dh_params_len;
        dh_params->dwMagic = BCRYPT_DH_PARAMETERS_MAGIC;
        dh_params->cbKeyLength = key_length_bytes;
        memcpy_with_be_padding((unsigned char *)dh_params +
                               sizeof(*dh_params),
                               key_length_bytes, p->bignum, p->length);
        memcpy_with_be_padding((unsigned char *)dh_params +
                               sizeof(*dh_params) + key_length_bytes,
                               key_length_bytes, g->bignum, g->length);

        status = BCryptSetProperty(dhctx->dh_handle, BCRYPT_DH_PARAMETERS,
                                   (PUCHAR)dh_params, dh_params_len, 0);
        if(hasAlgDHwithKDF == -1) {
            /* We know that the raw KDF is not supported, so discard this. */
            free(dh_params);
        }
        else {
            /* Pass ownership to dhctx; these parameters will be freed when
             * the context is destroyed. We need to keep the parameters more
             * easily available so that we have access to the `g` value when
             * _libssh2_dh_secret() is called later. */
            dhctx->dh_params = dh_params;
        }
        dh_params = NULL;

        if(!BCRYPT_SUCCESS(status)) {
            return -1;
        }

        status = BCryptFinalizeKeyPair(dhctx->dh_handle, 0);
        if(!BCRYPT_SUCCESS(status)) {
            return -1;
        }

        key_length_bytes = 0;
        if(hasAlgDHwithKDF == 1) {
            /* Now we need to extract the public portion of the key so that we
             * set it in the `public` bignum to satisfy our caller.
             * First measure up the size of the required buffer. */
            key_type = BCRYPT_DH_PUBLIC_BLOB;
        }
        else {
            /* We also need to extract the private portion of the key to
             * set it in the `*dhctx' bignum if the raw KDF is not supported.
             * First measure up the size of the required buffer. */
            key_type = BCRYPT_DH_PRIVATE_BLOB;
        }
        status = BCryptExportKey(dhctx->dh_handle, NULL, key_type,
                                 NULL, 0, &key_length_bytes, 0);
        if(!BCRYPT_SUCCESS(status)) {
            return -1;
        }

        dh_key_blob = (BCRYPT_DH_KEY_BLOB *)malloc(key_length_bytes);
        if(!dh_key_blob) {
            return -1;
        }

        status = BCryptExportKey(dhctx->dh_handle, NULL, key_type,
                                 (PUCHAR)dh_key_blob, key_length_bytes,
                                 &key_length_bytes, 0);
        if(!BCRYPT_SUCCESS(status)) {
            if(hasAlgDHwithKDF == 1) {
                /* We have no private data, because raw KDF is supported */
                free(dh_key_blob);
            }
            else { /* we may have potentially private data, use secure free */
                _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);
            }
            return -1;
        }

        if(hasAlgDHwithKDF == -1) {
            /* We know that the raw KDF is not supported, so discard this */
            BCryptDestroyKey(dhctx->dh_handle);
            dhctx->dh_handle = NULL;
        }

        /* BCRYPT_DH_PUBLIC_BLOB corresponds to a BCRYPT_DH_KEY_BLOB header
         * followed by the Modulus, Generator and Public data. Those components
         * each have equal size, specified by dh_key_blob->cbKey. */
        if(_libssh2_wincng_bignum_resize(public, dh_key_blob->cbKey)) {
            if(hasAlgDHwithKDF == 1) {
                /* We have no private data, because raw KDF is supported */
                free(dh_key_blob);
            }
            else { /* we may have potentially private data, use secure free */
                _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);
            }
            return -1;
        }

        /* Copy the public key data into the public bignum data buffer */
        memcpy(public->bignum, (unsigned char *)dh_key_blob +
                               sizeof(*dh_key_blob) +
                               2 * dh_key_blob->cbKey,
               dh_key_blob->cbKey);

        if(dh_key_blob->dwMagic == BCRYPT_DH_PRIVATE_MAGIC) {
            /* BCRYPT_DH_PRIVATE_BLOB additionally contains the Private data */
            dhctx->dh_privbn = _libssh2_wincng_bignum_init();
            if(!dhctx->dh_privbn) {
                _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);
                return -1;
            }
            if(_libssh2_wincng_bignum_resize(dhctx->dh_privbn,
                                             dh_key_blob->cbKey)) {
                _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);
                return -1;
            }

            /* Copy the private key data into the dhctx bignum data buffer */
            memcpy(dhctx->dh_privbn->bignum, (unsigned char *)dh_key_blob +
                                             sizeof(*dh_key_blob) +
                                             3 * dh_key_blob->cbKey,
                   dh_key_blob->cbKey);

            /* Make sure the private key is an odd number, because only
             * odd primes can be used with the RSA-based fallback while
             * DH itself does not seem to care about it being odd or not. */
            if(!(dhctx->dh_privbn->bignum[dhctx->dh_privbn->length-1] % 2)) {
                _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);
                /* discard everything first, then try again */
                _libssh2_dh_dtor(dhctx);
                _libssh2_dh_init(dhctx);
                continue;
            }
        }

        _libssh2_wincng_safe_free(dh_key_blob, key_length_bytes);

        return 0;
    }

    /* Generate x and e */
    dhctx->dh_privbn = _libssh2_wincng_bignum_init();
    if(!dhctx->dh_privbn)
        return -1;
    if(_libssh2_wincng_bignum_rand(dhctx->dh_privbn, (group_order*8)-1, 0, -1))
        return -1;
    if(_libssh2_wincng_bignum_mod_exp(public, g, dhctx->dh_privbn, p))
        return -1;

    return 0;
}

/* Computes the Diffie-Hellman secret from the previously created context
 * `*dhctx', the public key `f' from the other party and the same prime `p'
 * used at context creation. The result is stored in `secret'.  0 is returned
 * upon success, else -1.  */
int
_libssh2_dh_secret(_libssh2_dh_ctx *dhctx, _libssh2_bn *secret,
                   _libssh2_bn *f, _libssh2_bn *p)
{
    if(_libssh2_wincng.hAlgDH && _libssh2_wincng.hasAlgDHwithKDF != -1 &&
       dhctx->dh_handle && dhctx->dh_params && f) {
        BCRYPT_KEY_HANDLE peer_public = NULL;
        BCRYPT_SECRET_HANDLE agreement = NULL;
        ULONG secret_len_bytes = 0;
        NTSTATUS status;
        unsigned char *start, *end;
        BCRYPT_DH_KEY_BLOB *public_blob;
        ULONG key_length_bytes = max(f->length, dhctx->dh_params->cbKeyLength);
        ULONG public_blob_len = (ULONG)(sizeof(*public_blob) +
                                        3 * key_length_bytes);

        {
            /* Populate a BCRYPT_DH_KEY_BLOB; after the header follows the
             * Modulus, Generator and Public data. Those components must have
             * equal size in this representation. */
            unsigned char *dest;
            unsigned char *src;

            public_blob = (BCRYPT_DH_KEY_BLOB *)malloc(public_blob_len);
            if(!public_blob) {
                return -1;
            }
            public_blob->dwMagic = BCRYPT_DH_PUBLIC_MAGIC;
            public_blob->cbKey = key_length_bytes;

            dest = (unsigned char *)(public_blob + 1);
            src = (unsigned char *)(dhctx->dh_params + 1);

            /* Modulus (the p-value from the first call) */
            memcpy_with_be_padding(dest, key_length_bytes, src,
                                   dhctx->dh_params->cbKeyLength);
            /* Generator (the g-value from the first call) */
            memcpy_with_be_padding(dest + key_length_bytes, key_length_bytes,
                                   src + dhctx->dh_params->cbKeyLength,
                                   dhctx->dh_params->cbKeyLength);
            /* Public from the peer */
            memcpy_with_be_padding(dest + 2*key_length_bytes, key_length_bytes,
                                   f->bignum, f->length);
        }

        /* Import the peer public key information */
        status = BCryptImportKeyPair(_libssh2_wincng.hAlgDH, NULL,
                                     BCRYPT_DH_PUBLIC_BLOB, &peer_public,
                                     (PUCHAR)public_blob, public_blob_len, 0);
        if(!BCRYPT_SUCCESS(status)) {
            goto out;
        }

        /* Set up a handle that we can use to establish the shared secret
         * between ourselves (our saved dh_handle) and the peer. */
        status = BCryptSecretAgreement(dhctx->dh_handle, peer_public,
                                       &agreement, 0);
        if(!BCRYPT_SUCCESS(status)) {
            goto out;
        }

        /* Compute the size of the buffer that is needed to hold the derived
         * shared secret. */
        status = BCryptDeriveKey(agreement, BCRYPT_KDF_RAW_SECRET, NULL, NULL,
                                 0, &secret_len_bytes, 0);
        if(!BCRYPT_SUCCESS(status)) {
            if(status == STATUS_NOT_SUPPORTED) {
                _libssh2_wincng.hasAlgDHwithKDF = -1;
            }
            goto out;
        }

        /* Expand the secret bignum to be ready to receive the derived secret
         * */
        if(_libssh2_wincng_bignum_resize(secret, secret_len_bytes)) {
            status = (NTSTATUS)STATUS_NO_MEMORY;
            goto out;
        }

        /* And populate the secret bignum */
        status = BCryptDeriveKey(agreement, BCRYPT_KDF_RAW_SECRET, NULL,
                                 secret->bignum, secret_len_bytes,
                                 &secret_len_bytes, 0);
        if(!BCRYPT_SUCCESS(status)) {
            if(status == STATUS_NOT_SUPPORTED) {
                _libssh2_wincng.hasAlgDHwithKDF = -1;
            }
            goto out;
        }

        /* Counter to all the other data in the BCrypt APIs, the raw secret is
         * returned to us in host byte order, so we need to swap it to big
         * endian order. */
        start = secret->bignum;
        end = secret->bignum + secret->length - 1;
        while(start < end) {
            unsigned char tmp = *end;
            *end = *start;
            *start = tmp;
            start++;
            end--;
        }

        status = 0;
        _libssh2_wincng.hasAlgDHwithKDF = 1;

out:
        if(peer_public) {
            BCryptDestroyKey(peer_public);
        }
        if(agreement) {
            BCryptDestroySecret(agreement);
        }

        free(public_blob);

        if(status == STATUS_NOT_SUPPORTED &&
           _libssh2_wincng.hasAlgDHwithKDF == -1) {
            goto fb; /* fallback to RSA-based implementation */
        }
        return BCRYPT_SUCCESS(status) ? 0 : -1;
    }

fb:
    /* Compute the shared secret */
    return _libssh2_wincng_bignum_mod_exp(secret, f, dhctx->dh_privbn, p);
}

/* _libssh2_supported_key_sign_algorithms
 *
 * Return supported key hash algo upgrades, see crypto.h
 *
 */

const char *
_libssh2_supported_key_sign_algorithms(LIBSSH2_SESSION *session,
                                       unsigned char *key_method,
                                       size_t key_method_len)
{
    (void)session;

#if LIBSSH2_RSA_SHA2
    if(key_method_len == 7 &&
        memcmp(key_method, "ssh-rsa", key_method_len) == 0) {
        return "rsa-sha2-512,rsa-sha2-256"
#if LIBSSH2_RSA_SHA1
            ",ssh-rsa"
#endif
            ;
    }
#else
    (void)key_method;
    (void)key_method_len;
#endif

    return NULL;
}

#endif /* LIBSSH2_CRYPTO_C */
