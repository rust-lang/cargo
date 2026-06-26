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

#ifdef LIBSSH2_CRYPTO_C /* Compile this via crypto.c */

#include <stdlib.h>

#include <stdarg.h>
#include <alloca.h>
#include <sys/uio.h>

#include <arpa/inet.h>


#ifdef OS400_DEBUG
/* In debug mode, all system library errors cause an exception. */
#define set_EC_length(ec, length)   ((ec).Bytes_Provided =                  \
                                     (ec).Bytes_Available = 0)
#else
#define set_EC_length(ec, length)   ((ec).Bytes_Provided = (length))
#endif


/* Ensure va_list operations are not on an array. */
typedef struct {
    va_list     list;
}       valiststr;


typedef int (*loadkeyproc)(LIBSSH2_SESSION *session,
                           const unsigned char *data, unsigned int datalen,
                           const unsigned char *passphrase, void *loadkeydata);

/* Public key extraction data. */
typedef struct {
    const char *            method;
    const unsigned char *   data;
    unsigned int            length;
}       loadpubkeydata;


/* Support for ASN.1 elements. */

typedef struct {
    char *          header;         /* Pointer to header byte. */
    char *          beg;            /* Pointer to element data. */
    char *          end;            /* Pointer to 1st byte after element. */
    unsigned char   class;          /* ASN.1 element class. */
    unsigned char   tag;            /* ASN.1 element tag. */
    unsigned char   constructed;    /* Element is constructed. */
}       asn1Element;

#define ASN1_INTEGER        2
#define ASN1_BIT_STRING     3
#define ASN1_OCTET_STRING   4
#define ASN1_NULL           5
#define ASN1_OBJ_ID         6
#define ASN1_SEQ            16

#define ASN1_CONSTRUCTED    0x20

/* rsaEncryption OID: 1.2.840.113549.1.1.1 */
static unsigned char    OID_rsaEncryption[] =
                            {9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 1, 1, 1};
static int  sshrsapubkey(LIBSSH2_SESSION *session, char **sshpubkey,
                         asn1Element *params, asn1Element *key,
                         const char *method);

#if LIBSSH2_DSA != 0
/* dsaEncryption OID: 1.2.840.10040.4.1 */
static unsigned char    OID_dsaEncryption[] =
                            {7, 40 + 2, 0x86, 0x48, 0xCE, 0x38, 4, 1};
static int  sshdsapubkey(LIBSSH2_SESSION *session, char **sshpubkey,
                         asn1Element *params, asn1Element *key,
                         const char *method);
#endif

static unsigned char    OID_dhKeyAgreement[] =
                            {9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 1, 3, 1};


/* PKCS#5 support. */

typedef struct pkcs5params  pkcs5params;
struct pkcs5params {
    int         cipher;         /* Encryption cipher. */
    int         blocksize;      /* Cipher block size. */
    char        mode;           /* Block encryption mode. */
    char        padopt;         /* Pad option. */
    char        padchar;        /* Pad character. */
    int         (*kdf)(LIBSSH2_SESSION *session, char **dk,
                       const unsigned char *passphrase, pkcs5params *pkcs5);
    int         hash;           /* KDF hash algorithm. */
    size_t      hashlen;        /* KDF hash digest length. */
    char *      salt;           /* Salt. */
    size_t      saltlen;        /* Salt length. */
    char *      iv;             /* Initialization vector. */
    size_t      ivlen;          /* Initialization vector length. */
    int         itercount;      /* KDF iteration count. */
    int         dklen;          /* Derived key length (#bytes). */
    int         effkeysize;     /* RC2 effective key size (#bits) or 0. */
};

typedef struct pkcs5algo    pkcs5algo;
struct pkcs5algo {
    const unsigned char *   oid;
    int         (*parse)(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                         pkcs5algo *algo, asn1Element *param);
    int         cipher;         /* Encryption cipher. */
    size_t      blocksize;      /* Cipher block size. */
    char        mode;           /* Block encryption mode. */
    char        padopt;         /* Pad option. */
    char        padchar;        /* Pad character. */
    size_t      keylen;         /* Key length (#bytes). */
    int         hash;           /* Hash algorithm. */
    size_t      hashlen;        /* Hash digest length. */
    size_t      saltlen;        /* Salt length. */
    size_t      ivlen;          /* Initialisation vector length. */
    int         effkeysize;     /* RC2 effective key size (#bits) or 0. */
};

/* id-PBES2 OID: 1.2.840.113549.1.5.13 */
static const unsigned char  OID_id_PBES2[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x0D
};
static int  parse_pbes2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                        pkcs5algo *algo, asn1Element *param);
static const pkcs5algo  PBES2 = {
    OID_id_PBES2,   parse_pbes2,    0,  0,  '\0',   '\0',   '\0',   0,
    0,  0,  0,  0,  0
};

/* id-PBKDF2 OID: 1.2.840.113549.1.5.12 */
static const unsigned char  OID_id_PBKDF2[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x0C
};
static int  parse_pbkdf2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                         pkcs5algo *algo, asn1Element *param);
static const pkcs5algo  PBKDF2 = {
    OID_id_PBKDF2,  parse_pbkdf2,   0,  0,  '\0',   '\0',   '\0',
    SHA_DIGEST_LENGTH,  Qc3_SHA1,   SHA_DIGEST_LENGTH,  8,  8,  0
};

/* id-hmacWithSHA1 OID: 1.2.840.113549.2.7 */
static const unsigned char  OID_id_hmacWithSHA1[] = {
    8, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x02, 0x07
};
static int  parse_hmacWithSHA1(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                               pkcs5algo *algo, asn1Element *param);
static const pkcs5algo  hmacWithSHA1 = {
    OID_id_hmacWithSHA1,    parse_hmacWithSHA1, 0,  0,  '\0',   '\0',   '\0',
    SHA_DIGEST_LENGTH,  Qc3_SHA1,   SHA_DIGEST_LENGTH,  8,  8,  0
};

/* desCBC OID: 1.3.14.3.2.7 */
static const unsigned char  OID_desCBC[] = {5, 40 + 3, 0x0E, 0x03, 0x02, 0x07};
static int  parse_iv(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                     pkcs5algo *algo, asn1Element *param);
static const pkcs5algo  desCBC = {
    OID_desCBC, parse_iv,   Qc3_DES,    8,  Qc3_CBC,    Qc3_Pad_Counter,
   '\0',   8,   0,  0,  8,  8,  0
};

/* des-EDE3-CBC OID: 1.2.840.113549.3.7 */
static const unsigned char  OID_des_EDE3_CBC[] = {
    8, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x03, 0x07
};
static const pkcs5algo  des_EDE3_CBC = {
    OID_des_EDE3_CBC,   parse_iv,   Qc3_TDES,   8,  Qc3_CBC, Qc3_Pad_Counter,
    '\0',   24, 0,  0,  8,  8,  0
};

/* rc2CBC OID: 1.2.840.113549.3.2 */
static const unsigned char  OID_rc2CBC[] = {
    8, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x03, 0x02
};
static int  parse_rc2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                      pkcs5algo *algo, asn1Element *param);
static const pkcs5algo  rc2CBC = {
    OID_rc2CBC, parse_rc2,  Qc3_RC2,    8,  Qc3_CBC,    Qc3_Pad_Counter,
    '\0',   0,  0,  0,  8,  0,  32
};

static int  parse_pbes1(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                        pkcs5algo *algo, asn1Element *param);

#if LIBSSH2_MD5
/* pbeWithMD5AndDES-CBC OID: 1.2.840.113549.1.5.3 */
static const unsigned char  OID_pbeWithMD5AndDES_CBC[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x03
};
static const pkcs5algo  pbeWithMD5AndDES_CBC = {
    OID_pbeWithMD5AndDES_CBC,   parse_pbes1,    Qc3_DES,    8,  Qc3_CBC,
    Qc3_Pad_Counter,    '\0',   8,  Qc3_MD5,    MD5_DIGEST_LENGTH,  8,  0,  0
};

/* pbeWithMD5AndRC2-CBC OID: 1.2.840.113549.1.5.6 */
static const unsigned char  OID_pbeWithMD5AndRC2_CBC[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x06
};
static const pkcs5algo  pbeWithMD5AndRC2_CBC = {
    OID_pbeWithMD5AndRC2_CBC,   parse_pbes1,    Qc3_RC2,    8,  Qc3_CBC,
    Qc3_Pad_Counter,    '\0',   0,  Qc3_MD5,    MD5_DIGEST_LENGTH,  8,  0,  64
};
#endif

/* pbeWithSHA1AndDES-CBC OID: 1.2.840.113549.1.5.10 */
static const unsigned char  OID_pbeWithSHA1AndDES_CBC[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x0A
};
static const pkcs5algo  pbeWithSHA1AndDES_CBC = {
    OID_pbeWithSHA1AndDES_CBC,   parse_pbes1,    Qc3_DES,    8,  Qc3_CBC,
    Qc3_Pad_Counter,    '\0',   8,  Qc3_SHA1,   SHA_DIGEST_LENGTH,  8,  0, 0
};

/* pbeWithSHA1AndRC2-CBC OID: 1.2.840.113549.1.5.11 */
static const unsigned char  OID_pbeWithSHA1AndRC2_CBC[] = {
    9, 40 + 2, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x05, 0x0B
};
static const pkcs5algo  pbeWithSHA1AndRC2_CBC = {
    OID_pbeWithSHA1AndRC2_CBC,   parse_pbes1,    Qc3_RC2,    8,  Qc3_CBC,
    Qc3_Pad_Counter,    '\0',   0,  Qc3_SHA1,   SHA_DIGEST_LENGTH,  8,  0,  64
};

/* rc5-CBC-PAD OID: 1.2.840.113549.3.9: RC5 not implemented in Qc3. */
/* pbeWithMD2AndDES-CBC OID: 1.2.840.113549.1.5.1: MD2 not implemented. */
/* pbeWithMD2AndRC2-CBC OID: 1.2.840.113549.1.5.4: MD2 not implemented. */

static const pkcs5algo *    pbestable[] = {
#if LIBSSH2_MD5
    &pbeWithMD5AndDES_CBC,
    &pbeWithMD5AndRC2_CBC,
#endif
    &pbeWithSHA1AndDES_CBC,
    &pbeWithSHA1AndRC2_CBC,
    &PBES2,
    NULL
};

static const pkcs5algo *    pbkdf2table[] = {
    &PBKDF2,
    NULL
};

static const pkcs5algo *    pbes2enctable[] = {
    &desCBC,
    &des_EDE3_CBC,
    &rc2CBC,
    NULL
};

static const pkcs5algo *    kdf2prftable[] = {
    &hmacWithSHA1,
    NULL
};


/* Public key extraction support. */
static struct {
    unsigned char *oid;
    int             (*sshpubkey)(LIBSSH2_SESSION *session, char **pubkey,
                                 asn1Element *params, asn1Element *key,
                                 const char *method);
    const char *    method;
}       pka[] = {
#if LIBSSH2_RSA != 0
    {   OID_rsaEncryption,  sshrsapubkey,   "ssh-rsa"   },
#endif
#if LIBSSH2_DSA != 0
    {   OID_dsaEncryption,  sshdsapubkey,   "ssh-dss"   },
#endif
    {   NULL,               NULL,           NULL        }
};

/* Define ASCII strings. */
static const char   beginencprivkeyhdr[] =
                                    "-----BEGIN ENCRYPTED PRIVATE KEY-----";
static const char   endencprivkeyhdr[] = "-----END ENCRYPTED PRIVATE KEY-----";
static const char   beginprivkeyhdr[] = "-----BEGIN PRIVATE KEY-----";
static const char   endprivkeyhdr[] = "-----END PRIVATE KEY-----";
static const char   beginrsaprivkeyhdr[] = "-----BEGIN RSA PRIVATE KEY-----";
static const char   endrsaprivkeyhdr[] = "-----END RSA PRIVATE KEY-----";
static const char   fopenrmode[] = "r";
static const char   fopenrbmode[] = "rb";


/* The rest of character literals in this module are in EBCDIC. */
#pragma convert(37)

#include <qusec.h>
#include <qc3prng.h>
#include <qc3dtaen.h>
#include <qc3dtade.h>
#include <qc3ctx.h>
#include <qc3hash.h>
#include <qc3hmac.h>
#include <qc3pbext.h>
#include <qc3sigvr.h>
#include <qc3sigcl.h>
#include <qc3pbext.h>
#include <qc3dh.h>

static Qc3_Format_KEYD0100_T    nulltoken = {""};

static int      zero = 0;
static int      rsaprivate[] = { Qc3_RSA_Private };
static char     anycsp[] = { Qc3_Any_CSP };
static char     binstring[] = { Qc3_Bin_String };
static char     berstring[] = { Qc3_BER_String };
static char     qc3clear[] = { Qc3_Clear };

static const Qus_EC_t ecnull = {0};     /* Error causes an exception. */

static asn1Element  lastbytebitcount = {
    (char *) &zero, NULL, (char *) &zero + 1
};


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: big numbers support.
 *
 *******************************************************************/

int
_libssh2_random(unsigned char *buf, size_t len)
{
    Qus_EC_t errcode;

    set_EC_length(errcode, sizeof(errcode));
    Qc3GenPRNs(buf, len,
        Qc3PRN_TYPE_NORMAL, Qc3PRN_NO_PARITY, (char *) &errcode);
    return errcode.Bytes_Available ? -1 : 0;
}

_libssh2_bn *
_libssh2_bn_init(void)
{
    _libssh2_bn *bignum;

    bignum = (_libssh2_bn *) malloc(sizeof(*bignum));
    if(bignum) {
        bignum->bignum = NULL;
        bignum->length = 0;
    }

    return bignum;
}

void
_libssh2_bn_free(_libssh2_bn *bn)
{
    if(bn) {
        if(bn->bignum) {
            if(bn->length)
                _libssh2_explicit_zero(bn->bignum, bn->length);

            free(bn->bignum);
        }

        free((char *) bn);
    }
}

static int
_libssh2_bn_resize(_libssh2_bn *bn, size_t newlen)
{
    unsigned char *bignum;

    if(!bn)
        return -1;
    if(newlen == bn->length)
        return 0;

    if(!bn->bignum)
        bignum = (unsigned char *) malloc(newlen);
    else {
        if(newlen < bn->length)
            _libssh2_explicit_zero(bn->bignum + newlen, bn->length - newlen);

        if(!newlen) {
            free((char *) bn->bignum);
            bn->bignum = NULL;
            bn->length = 0;
            return 0;
        }
        bignum = (unsigned char *) realloc((char *) bn->bignum, newlen);
    }

    if(!bignum)
        return -1;

    if(newlen > bn->length)
        memset((char *) bignum + bn->length, 0, newlen - bn->length);

    bn->bignum = bignum;
    bn->length = newlen;
    return 0;
}

unsigned long
_libssh2_bn_bits(_libssh2_bn *bn)
{
    unsigned int i;
    unsigned char b;

    if(bn && bn->bignum) {
        for(i = bn->length; i--;) {
            b = bn->bignum[i];
            if(b) {
                i *= 8;
                do {
                    i++;
                } while(b >>= 1);
                return i;
            }
        }
    }

    return 0;
}

int
_libssh2_bn_from_bin(_libssh2_bn *bn, size_t len, const unsigned char *val)
{
    size_t i;

    if(!bn || (len && !val))
        return -1;

    for(; len && !*val; len--)
        val++;

    if(_libssh2_bn_resize(bn, len))
        return -1;

    for(i = len; i--;)
        bn->bignum[i] = *val++;

    return 0;
}

int
_libssh2_bn_set_word(_libssh2_bn *bn, unsigned long val)
{
    val = htonl(val);
    return _libssh2_bn_from_bin(bn, sizeof(val), (unsigned char *) &val);
}

int
_libssh2_bn_to_bin(_libssh2_bn *bn, unsigned char *val)
{
    int i;

    if(!bn || !val)
        return -1;

    for(i = bn->length; i--;)
        *val++ = bn->bignum[i];

    return 0;
}

static int
_libssh2_bn_from_bn(_libssh2_bn *to, _libssh2_bn *from)
{
    int i;

    if(!to || !from)
        return -1;

    if(_libssh2_bn_resize(to, from->length))
        return -1;

    for(i = to->length; i--;)
        to->bignum[i] = from->bignum[i];

    return 0;
}


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: ASN.1 support.
 *
 *******************************************************************/

static char *
getASN1Element(asn1Element *elem, char *beg, char *end)
{
    unsigned char b;
    unsigned long len;
    asn1Element lelem;

    /* Get a single ASN.1 element into `elem', parse ASN.1 string at `beg'
     * ending at `end'.
     * Returns a pointer in source string after the parsed element, or NULL
     * if an error occurs.
     */

    if(beg >= end || !*beg)
        return NULL;

    /* Process header byte. */
    elem->header = beg;
    b = (unsigned char) *beg++;
    elem->constructed = (b & 0x20) != 0;
    elem->class = (b >> 6) & 3;
    b &= 0x1F;
    if(b == 0x1F)
        return NULL;            /* Long tag values not supported here. */
    elem->tag = b;

    /* Process length. */
    if(beg >= end)
        return NULL;
    b = (unsigned char) *beg++;
    if(!(b & 0x80))
        len = b;
    else if(!(b &= 0x7F)) {
        /* Unspecified length. Since we have all the data, we can determine the
         * effective length by skipping element until an end element is
         * found.
         */
        if(!elem->constructed)
            return NULL;
        elem->beg = beg;
        while(beg < end && *beg) {
            beg = getASN1Element(&lelem, beg, end);
        if(!beg)
            return NULL;
        }
        if(beg >= end)
            return NULL;
        elem->end = beg;
        return beg + 1;
    }
    else if(beg + b > end)
        return NULL;                        /* Does not fit in source. */
    else {
        /* Get long length. */
        len = 0;
        do {
            if(len & 0xFF000000L)
                return NULL;    /* Lengths > 32 bits are not supported. */
            len = (len << 8) | (unsigned char) *beg++;
        } while(--b);
    }
    if((unsigned long) (end - beg) < len)
        return NULL;            /* Element data does not fit in source. */
    elem->beg = beg;
    elem->end = beg + len;
    return elem->end;
}

static asn1Element *
asn1_new(unsigned int type, unsigned int length)
{
    asn1Element *e;
    unsigned int hdrl = 2;
    unsigned int i;
    unsigned char *buf;

    e = (asn1Element *) malloc(sizeof(*e));

    if(e) {
        if(length >= 0x80)
            for(i = length; i; i >>= 8)
                hdrl++;

        buf = (unsigned char *) malloc(hdrl + length);

        if(buf) {
            e->header = buf;
            e->beg = buf + hdrl;
            e->end = e->beg + length;
            e->class = (type >> 6) & 0x03;
            e->tag = type & 0x1F;
            e->constructed = (type >> 5) & 0x01;
            e->header[0] = type;

            if(length < 0x80)
                e->header[1] = length;
            else {
                e->header[1] = (hdrl - 2) | 0x80;
                do {
                    e->header[--hdrl] = length;
                    length >>= 8;
                } while(length);
            }
        }
        else {
            free((char *) e);
            e = NULL;
        }
    }

    return e;
}

static asn1Element *
asn1_new_from_bytes(const unsigned char *data, unsigned int length)
{
    asn1Element *e;
    asn1Element et;

    getASN1Element(&et,
                   (unsigned char *) data, (unsigned char *) data + length);
    e = asn1_new(et.tag, et.end - et.beg);

    if(e)
        memcpy(e->header, data, e->end - e->header);

    return e;
}

static void
asn1delete(asn1Element *e)
{
    if(e) {
        if(e->header)
            free((char *) e->header);
        free((char *) e);
    }
}

static asn1Element *
asn1uint(_libssh2_bn *bn)
{
    asn1Element *e;
    int bits;
    int length;
    unsigned char *p;

    if(!bn)
        return NULL;

    bits = _libssh2_bn_bits(bn);
    length = (bits + 8) >> 3;
    e = asn1_new(ASN1_INTEGER, length);

    if(e) {
        p = e->beg;
        if(!(bits & 0x07))
            *p++ = 0;
        _libssh2_bn_to_bin(bn, p);
    }

    return e;
}

static asn1Element *
asn1containerv(unsigned int type, valiststr args)
{
    valiststr va;
    asn1Element *e;
    asn1Element *p;
    unsigned char *bp;
    unsigned int length = 0;

    memcpy((char *) &va, (char *) &args, sizeof(args));
    while((p = va_arg(va.list, asn1Element *)))
        length += p->end - p->header;
    va_end(va.list);
    e = asn1_new(type, length);
    if(e) {
        bp = e->beg;
        while((p = va_arg(args.list, asn1Element *))) {
            memcpy(bp, p->header, p->end - p->header);
            bp += p->end - p->header;
        }
    }
    return e;
}

/* VARARGS1 */
static asn1Element *
asn1container(unsigned int type, ...)
{
    valiststr va;
    asn1Element *e;

    va_start(va.list, type);
    e = asn1containerv(type, va);
    va_end(va.list);
    return e;
}

static asn1Element *
asn1bytes(unsigned int type, const unsigned char *bytes, unsigned int length)
{
    asn1Element *e;

    e = asn1_new(type, length);
    if(e && length)
        memcpy(e->beg, bytes, length);
    return e;
}

static asn1Element *
rsapublickey(_libssh2_bn *e, _libssh2_bn *m)
{
    asn1Element *publicexponent;
    asn1Element *modulus;
    asn1Element *rsapubkey;

    /* Build a PKCS#1 RSAPublicKey. */

    modulus = asn1uint(m);
    publicexponent = asn1uint(e);
    rsapubkey = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                              modulus, publicexponent, NULL);
    asn1delete(modulus);
    asn1delete(publicexponent);

    if(!modulus || !publicexponent) {
        asn1delete(rsapubkey);
        rsapubkey = NULL;
    }

    return rsapubkey;
}

static asn1Element *
rsaprivatekey(_libssh2_bn *e, _libssh2_bn *m, _libssh2_bn *d,
              _libssh2_bn *p, _libssh2_bn *q,
              _libssh2_bn *exp1, _libssh2_bn *exp2, _libssh2_bn *coeff)
{
    asn1Element *version;
    asn1Element *modulus;
    asn1Element *publicexponent;
    asn1Element *privateexponent;
    asn1Element *prime1;
    asn1Element *prime2;
    asn1Element *exponent1;
    asn1Element *exponent2;
    asn1Element *coefficient;
    asn1Element *rsaprivkey;

    /* Build a PKCS#1 RSAPrivateKey. */
    version = asn1bytes(ASN1_INTEGER, "\0", 1);
    modulus = asn1uint(m);
    publicexponent = asn1uint(e);
    privateexponent = asn1uint(d);
    prime1 = asn1uint(p);
    prime2 = asn1uint(q);
    exponent1 = asn1uint(exp1);
    exponent2 = asn1uint(exp2);
    coefficient = asn1uint(coeff);
    rsaprivkey = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED, version, modulus,
                               publicexponent, privateexponent, prime1, prime2,
                               exponent1, exponent2, coefficient, NULL);
    asn1delete(version);
    asn1delete(modulus);
    asn1delete(publicexponent);
    asn1delete(privateexponent);
    asn1delete(prime1);
    asn1delete(prime2);
    asn1delete(exponent1);
    asn1delete(exponent2);
    asn1delete(coefficient);

    if(!version || !modulus || !publicexponent || !privateexponent ||
        !prime1 || !prime2 || !exponent1 || !exponent2 || !coefficient) {
        asn1delete(rsaprivkey);
        rsaprivkey = NULL;
    }

    return rsaprivkey;
}

static asn1Element *
subjectpublickeyinfo(asn1Element *pubkey, const unsigned char *algo,
                     asn1Element *parameters)
{
    asn1Element *subjpubkey;
    asn1Element *algorithm;
    asn1Element *algorithmid;
    asn1Element *subjpubkeyinfo;
    unsigned int algosize = *algo++;

    algorithm = asn1bytes(ASN1_OBJ_ID, algo, algosize);
    algorithmid = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                                algorithm, parameters, NULL);
    subjpubkey = asn1container(ASN1_BIT_STRING, &lastbytebitcount,
                               pubkey, NULL);
    subjpubkeyinfo = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                                   algorithmid, subjpubkey, NULL);
    asn1delete(algorithm);
    asn1delete(algorithmid);
    asn1delete(subjpubkey);
    if(!algorithm || !algorithmid || !subjpubkey) {
        asn1delete(subjpubkeyinfo);
        subjpubkeyinfo = NULL;
    }
    return subjpubkeyinfo;
}

static asn1Element *
rsasubjectpublickeyinfo(asn1Element *pubkey)
{
    asn1Element *parameters;
    asn1Element *subjpubkeyinfo;

    parameters = asn1bytes(ASN1_NULL, NULL, 0);
    subjpubkeyinfo = subjectpublickeyinfo(pubkey,
                                          OID_rsaEncryption, parameters);
    asn1delete(parameters);
    if(!parameters) {
        asn1delete(subjpubkeyinfo);
        subjpubkeyinfo = NULL;
    }
    return subjpubkeyinfo;
}

static asn1Element *
privatekeyinfo(asn1Element *privkey, const unsigned char *algo,
               asn1Element *parameters)
{
    asn1Element *version;
    asn1Element *privatekey;
    asn1Element *algorithm;
    asn1Element *privatekeyalgorithm;
    asn1Element *privkeyinfo;
    unsigned int algosize = *algo++;

    /* Build a PKCS#8 PrivateKeyInfo. */
    version = asn1bytes(ASN1_INTEGER, "\0", 1);
    algorithm = asn1bytes(ASN1_OBJ_ID, algo, algosize);
    privatekeyalgorithm = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                                        algorithm, parameters, NULL);
    privatekey = asn1container(ASN1_OCTET_STRING, privkey, NULL);
    privkeyinfo = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED, version,
                                privatekeyalgorithm, privatekey, NULL);
    asn1delete(version);
    asn1delete(algorithm);
    asn1delete(privatekeyalgorithm);
    if(!version || !algorithm || !privatekeyalgorithm) {
        asn1delete(privkeyinfo);
        privkeyinfo = NULL;
    }
    return privkeyinfo;
}

static asn1Element *
rsaprivatekeyinfo(asn1Element *privkey)
{
    asn1Element *parameters;
    asn1Element *privkeyinfo;

    parameters = asn1bytes(ASN1_NULL, NULL, 0);
    privkeyinfo = privatekeyinfo(privkey, OID_rsaEncryption, parameters);
    asn1delete(parameters);
    if(!parameters) {
        asn1delete(privkeyinfo);
        privkeyinfo = NULL;
    }
    return privkeyinfo;
}


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: crypto context support.
 *
 *******************************************************************/

static _libssh2_os400qc3_crypto_ctx *
libssh2_init_crypto_ctx(_libssh2_os400qc3_crypto_ctx *ctx)
{
    if(!ctx)
        ctx = (_libssh2_os400qc3_crypto_ctx *) malloc(sizeof(*ctx));

    if(ctx) {
        memset((char *) ctx, 0, sizeof(*ctx));
        ctx->hash.Final_Op_Flag = Qc3_Continue;
    }

    return ctx;
}

static int
null_token(const char *token)
{
    return !memcmp(token, nulltoken.Key_Context_Token,
                   sizeof(nulltoken.Key_Context_Token));
}

void
_libssh2_os400qc3_crypto_dtor(_libssh2_os400qc3_crypto_ctx *x)
{
    if(!x)
        return;
    if(!null_token(x->hash.Alg_Context_Token)) {
        Qc3DestroyAlgorithmContext(x->hash.Alg_Context_Token,
                                   (char *) &ecnull);
        memset(x->hash.Alg_Context_Token, 0,
               sizeof(x->hash.Alg_Context_Token));
    }
    if(!null_token(x->key.Key_Context_Token)) {
        Qc3DestroyKeyContext(x->key.Key_Context_Token, (char *) &ecnull);
        memset(x->key.Key_Context_Token, 0,
               sizeof(x->key.Key_Context_Token));
    }
    if(x->kek) {
        _libssh2_os400qc3_crypto_dtor(x->kek);
        free((char *) x->kek);
        x->kek = NULL;
    }
}

/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: hash algorithms support.
 *
 *******************************************************************/

int
_libssh2_os400qc3_hash_init(Qc3_Format_ALGD0100_T *x, unsigned int algorithm)
{
    Qc3_Format_ALGD0500_T algd;
    Qus_EC_t errcode;

    if(!x)
        return 0;

    memset((char *) x, 0, sizeof(*x));
    x->Final_Op_Flag = Qc3_Continue;
    algd.Hash_Alg = algorithm;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CreateAlgorithmContext((char *) &algd, Qc3_Alg_Hash,
                              x->Alg_Context_Token, &errcode);
    return errcode.Bytes_Available ? 0 : 1;
}

int
_libssh2_os400qc3_hash_update(Qc3_Format_ALGD0100_T *ctx,
                              const unsigned char *data, int len)
{
    char dummy[64];
    Qus_EC_t errcode;

    ctx->Final_Op_Flag = Qc3_Continue;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateHash((char *) data, &len, Qc3_Data, (char *) ctx,
                     Qc3_Alg_Token, anycsp, NULL, dummy, &errcode);
    return errcode.Bytes_Available ? 0 : 1;
}

int
_libssh2_os400qc3_hash_final(Qc3_Format_ALGD0100_T *ctx, unsigned char *out)
{
    char data;
    Qus_EC_t errcode;

    ctx->Final_Op_Flag = Qc3_Final;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateHash(&data, &zero, Qc3_Data, (char *) ctx, Qc3_Alg_Token,
                     anycsp, NULL, (char *) out, &errcode);
    Qc3DestroyAlgorithmContext(ctx->Alg_Context_Token, (char *) &ecnull);
    memset(ctx->Alg_Context_Token, 0, sizeof(ctx->Alg_Context_Token));
    return errcode.Bytes_Available ? 0 : 1;
}

int
_libssh2_os400qc3_hash(const unsigned char *message, unsigned long len,
                       unsigned char *out, unsigned int algo)
{
    Qc3_Format_ALGD0100_T ctx;

    if(!_libssh2_os400qc3_hash_init(&ctx, algo) ||
       !_libssh2_os400qc3_hash_update(&ctx, message, len) ||
       !_libssh2_os400qc3_hash_final(&ctx, out))
        return 1;

    return 0;
}

static int
libssh2_os400qc3_hmac_init(_libssh2_os400qc3_crypto_ctx *ctx,
                           int algo, size_t minkeylen, void *key, int keylen)
{
    Qus_EC_t errcode;

    if(keylen < minkeylen) {
        char *lkey = alloca(minkeylen);

        /* Pad key with zeroes if too short. */
        if(!lkey)
            return 0;
        memcpy(lkey, (char *) key, keylen);
        memset(lkey + keylen, 0, minkeylen - keylen);
        key = (void *) lkey;
        keylen = minkeylen;
    }
    if(!_libssh2_os400qc3_hash_init(&ctx->hash, algo))
        return 0;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CreateKeyContext((char *) key, &keylen, binstring, &algo, qc3clear,
                        NULL, NULL, ctx->key.Key_Context_Token,
                        (char *) &errcode);
    return errcode.Bytes_Available ? 0 : 1;
}

int _libssh2_hmac_ctx_init(libssh2_hmac_ctx *ctx)
{
    memset((char *) ctx, 0, sizeof(libssh2_hmac_ctx));
    return 1;
}

#if LIBSSH2_MD5
int _libssh2_hmac_md5_init(libssh2_hmac_ctx *ctx,
                           void *key, size_t keylen)
{
    return libssh2_os400qc3_hmac_init(ctx, Qc3_MD5,                     \
                                      MD5_DIGEST_LENGTH,                \
                                      key, keylen);
}
#endif

int _libssh2_hmac_sha1_init(libssh2_hmac_ctx *ctx,
                            void *key, size_t keylen)
{
    return libssh2_os400qc3_hmac_init(ctx, Qc3_SHA1,                    \
                                      SHA_DIGEST_LENGTH,                \
                                      key, keylen);
}

int _libssh2_hmac_sha256_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    return libssh2_os400qc3_hmac_init(ctx, Qc3_SHA256,                  \
                                      SHA256_DIGEST_LENGTH,             \
                                      key, keylen);
}

int _libssh2_hmac_sha512_init(libssh2_hmac_ctx *ctx,
                              void *key, size_t keylen)
{
    return libssh2_os400qc3_hmac_init(ctx, Qc3_SHA512,                  \
                                      SHA512_DIGEST_LENGTH,             \
                                      key, keylen);
}

int _libssh2_hmac_update(libssh2_hmac_ctx *ctx,
                         const void *data, size_t datalen)
{
    char dummy[64];
    int len = (int) datalen;
    Qus_EC_t errcode;

    ctx->hash.Final_Op_Flag = Qc3_Continue;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateHMAC((char *) data, &len, Qc3_Data, (char *) &ctx->hash,
                     Qc3_Alg_Token, ctx->key.Key_Context_Token, Qc3_Key_Token,
                     anycsp, NULL, dummy, (char *) &errcode);
    return errcode.Bytes_Available ? 0 : 1;
}

int _libssh2_hmac_final(libssh2_hmac_ctx *ctx, void *out)
{
    char data;
    Qus_EC_t errcode;

    ctx->hash.Final_Op_Flag = Qc3_Final;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateHMAC((char *) data, &zero, Qc3_Data, (char *) &ctx->hash,
                     Qc3_Alg_Token, ctx->key.Key_Context_Token, Qc3_Key_Token,
                     anycsp, NULL, (char *) out, (char *) &errcode);
    return errcode.Bytes_Available ? 0 : 1;
}

void _libssh2_hmac_cleanup(libssh2_hmac_ctx *ctx)
{
    _libssh2_os400qc3_crypto_dtor(ctx);
}

/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: cipher algorithms support.
 *
 *******************************************************************/

int
_libssh2_cipher_init(_libssh2_cipher_ctx *h, _libssh2_cipher_type(algo),
                     unsigned char *iv, unsigned char *secret, int encrypt)
{
    Qc3_Format_ALGD0200_T algd;
    Qus_EC_t errcode;

    (void) encrypt;

    if(!h)
        return -1;

    libssh2_init_crypto_ctx(h);
    algd.Block_Cipher_Alg = algo.algo;
    algd.Block_Length = algo.size;
    algd.Mode = algo.mode;
    algd.Pad_Option = Qc3_No_Pad;
    algd.Pad_Character = 0;
    algd.Reserved = 0;
    algd.MAC_Length = 0;
    algd.Effective_Key_Size = 0;
    memset(algd.Init_Vector, 0, sizeof(algd.Init_Vector));
    if(algo.mode != Qc3_ECB && algo.size)
        memcpy(algd.Init_Vector, iv, algo.size);
    set_EC_length(errcode, sizeof(errcode));
    Qc3CreateAlgorithmContext((char *) &algd, algo.fmt,
                              h->hash.Alg_Context_Token, &errcode);
    if(errcode.Bytes_Available)
        return -1;
    Qc3CreateKeyContext((char *) secret, &algo.keylen, binstring,
                        &algo.algo, qc3clear, NULL, NULL,
                        h->key.Key_Context_Token, (char *) &errcode);
    if(errcode.Bytes_Available) {
        _libssh2_os400qc3_crypto_dtor(h);
        return -1;
    }

    return 0;
}

int
_libssh2_cipher_crypt(_libssh2_cipher_ctx *ctx,
                      _libssh2_cipher_type(algo),
                      int encrypt, unsigned char *block, size_t blocksize,
                      int firstlast)
{
    Qus_EC_t errcode;
    int outlen;
    int blksize = blocksize;

    (void) algo;

    set_EC_length(errcode, sizeof(errcode));
    if(encrypt)
        Qc3EncryptData((char *) block, &blksize, Qc3_Data,
                       ctx->hash.Alg_Context_Token, Qc3_Alg_Token,
                       ctx->key.Key_Context_Token, Qc3_Key_Token, anycsp, NULL,
                       (char *) block, &blksize, &outlen, (char *) &errcode);
    else
        Qc3DecryptData((char *) block, &blksize,
                       ctx->hash.Alg_Context_Token, Qc3_Alg_Token,
                       ctx->key.Key_Context_Token, Qc3_Key_Token, anycsp, NULL,
                       (char *) block, &blksize, &outlen, (char *) &errcode);

    return errcode.Bytes_Available ? -1 : 0;
}


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: RSA support.
 *
 *******************************************************************/

int
_libssh2_rsa_new(libssh2_rsa_ctx **rsa,
                 const unsigned char *edata, unsigned long elen,
                 const unsigned char *ndata, unsigned long nlen,
                 const unsigned char *ddata, unsigned long dlen,
                 const unsigned char *pdata, unsigned long plen,
                 const unsigned char *qdata, unsigned long qlen,
                 const unsigned char *e1data, unsigned long e1len,
                 const unsigned char *e2data, unsigned long e2len,
                 const unsigned char *coeffdata, unsigned long coefflen)
{
    libssh2_rsa_ctx *ctx;
    _libssh2_bn *e = _libssh2_bn_init_from_bin();
    _libssh2_bn *n = _libssh2_bn_init_from_bin();
    _libssh2_bn *d = NULL;
    _libssh2_bn *p = NULL;
    _libssh2_bn *q = NULL;
    _libssh2_bn *e1 = NULL;
    _libssh2_bn *e2 = NULL;
    _libssh2_bn *coeff = NULL;
    asn1Element *key = NULL;
    asn1Element *structkey = NULL;
    int keytype;
    int ret = 0;
    int i;

    ctx = libssh2_init_crypto_ctx(NULL);
    if(!ctx)
        ret = -1;
    if(!ret) {
        _libssh2_bn_from_bin(e, elen, edata);
        _libssh2_bn_from_bin(n, nlen, ndata);
        if(!e || !n)
            ret = -1;
    }
    if(!ret && ddata) {
        /* Private key. */
        d = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(d, dlen, ddata);
        p = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(p, plen, pdata);
        q = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(q, qlen, qdata);
        e1 = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(e1, e1len, e1data);
        e2 = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(e2, e2len, e2data);
        coeff = _libssh2_bn_init_from_bin();
        _libssh2_bn_from_bin(coeff, coefflen, coeffdata);
        if(!d || !p || !q ||!e1 || !e2 || !coeff)
            ret = -1;

        if(!ret) {
            /* Build a PKCS#8 private key. */
            key = rsaprivatekey(e, n, d, p, q, e1, e2, coeff);
            structkey = rsaprivatekeyinfo(key);
        }
        keytype = Qc3_RSA_Private;
    }
    else if(!ret) {
        key = rsapublickey(e, n);
        structkey = rsasubjectpublickeyinfo(key);
        keytype = Qc3_RSA_Public;
    }
    if(!key || !structkey)
        ret = -1;

    /* Create the key context. */
    if(!ret) {
        Qus_EC_t errcode;

        set_EC_length(errcode, sizeof(errcode));
        i = structkey->end - structkey->header;
        Qc3CreateKeyContext(structkey->header, &i, berstring, &keytype,
                            qc3clear, NULL, NULL, ctx->key.Key_Context_Token,
                            (char *) &errcode);
        if(errcode.Bytes_Available)
            ret = -1;
    }

    _libssh2_bn_free(e);
    _libssh2_bn_free(n);
    _libssh2_bn_free(d);
    _libssh2_bn_free(p);
    _libssh2_bn_free(q);
    _libssh2_bn_free(e1);
    _libssh2_bn_free(e2);
    _libssh2_bn_free(coeff);
    asn1delete(key);
    asn1delete(structkey);
    if(ret && ctx) {
        _libssh2_rsa_free(ctx);
        ctx = NULL;
    }
    *rsa = ctx;
    return ret;
}


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: Diffie-Hellman support.
 *
 *******************************************************************/

void
_libssh2_os400qc3_dh_init(_libssh2_dh_ctx *dhctx)
{
    memset((char *) dhctx, 0, sizeof(*dhctx));
}

int
_libssh2_os400qc3_dh_key_pair(_libssh2_dh_ctx *dhctx, _libssh2_bn *public,
                              _libssh2_bn *g, _libssh2_bn *p, int group_order)
{
    asn1Element *prime;
    asn1Element *base;
    asn1Element *dhparameter;
    asn1Element *dhkeyagreement;
    asn1Element *pkcs3;
    int pkcs3len;
    char *pubkey;
    int pubkeysize;
    int pubkeylen;
    Qus_EC_t errcode;

    (void) group_order;

    /* Build the PKCS#3 structure. */

    base = asn1uint(g);
    prime = asn1uint(p);
    dhparameter = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                                prime, base, NULL);
    asn1delete(base);
    asn1delete(prime);
    dhkeyagreement = asn1bytes(ASN1_OBJ_ID,
                               OID_dhKeyAgreement + 1, OID_dhKeyAgreement[0]);
    pkcs3 = asn1container(ASN1_SEQ | ASN1_CONSTRUCTED,
                          dhkeyagreement, dhparameter, NULL);
    asn1delete(dhkeyagreement);
    asn1delete(dhparameter);
    if(!base || !prime || !dhparameter ||
        !dhkeyagreement || !dhparameter || !pkcs3) {
        asn1delete(pkcs3);
        return -1;
    }
    pkcs3len = pkcs3->end - pkcs3->header;
    pubkeysize = (_libssh2_bn_bits(p) + 7) >> 3;
    pubkey = alloca(pubkeysize);
    set_EC_length(errcode, sizeof(errcode));
    Qc3GenDHKeyPair((char *) pkcs3->header, &pkcs3len, anycsp, NULL,
                    dhctx->token, pubkey, &pubkeysize, &pubkeylen, &errcode);
    asn1delete(pkcs3);
    if(errcode.Bytes_Available)
        return -1;
    return _libssh2_bn_from_bin(public, pubkeylen, (unsigned char *) pubkey);
}

int
_libssh2_os400qc3_dh_secret(_libssh2_dh_ctx *dhctx, _libssh2_bn *secret,
                            _libssh2_bn *f, _libssh2_bn *p)
{
    char *pubkey;
    int pubkeysize;
    char *secretbuf;
    int secretbufsize;
    int secretbuflen;
    Qus_EC_t errcode;

    pubkeysize = (_libssh2_bn_bits(f) + 7) >> 3;
    pubkey = alloca(pubkeysize);
    _libssh2_bn_to_bin(f, pubkey);
    secretbufsize = (_libssh2_bn_bits(p) + 7) >> 3;
    secretbuf = alloca(pubkeysize);
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateDHSecretKey(dhctx->token, pubkey, &pubkeysize,
                            secretbuf, &secretbufsize, &secretbuflen,
                            &errcode);
    if(errcode.Bytes_Available)
        return -1;
    return _libssh2_bn_from_bin(secret,
                                secretbuflen, (unsigned char *) secretbuf);
}

void
_libssh2_os400qc3_dh_dtor(_libssh2_dh_ctx *dhctx)
{
    if(!null_token(dhctx->token)) {
        Qc3DestroyAlgorithmContext(dhctx->token, (char *) &ecnull);
        memset((char *) dhctx, 0, sizeof(*dhctx));
    }
}


/*******************************************************************
 *
 * OS/400 QC3 crypto-library backend: PKCS#5 supplement.
 *
 *******************************************************************/

static int
oidcmp(const asn1Element *e, const unsigned char *oid)
{
    int i = e->end - e->beg - *oid++;

    if(*e->header != ASN1_OBJ_ID)
        return -2;
    if(!i)
        i = memcmp(e->beg, oid, oid[-1]);
    return i;
}

static int
asn1getword(asn1Element *e, unsigned long *v)
{
    unsigned long a;
    const unsigned char *cp;

    if(*e->header != ASN1_INTEGER)
        return -1;
    for(cp = e->beg; cp < e->end && !*cp; cp++)
        ;
    if(e->end - cp > sizeof(a))
        return -1;
    for(a = 0; cp < e->end; cp++)
        a = (a << 8) | *cp;
    *v = a;
    return 0;
}

static int
pbkdf1(LIBSSH2_SESSION *session, char **dk, const unsigned char *passphrase,
       pkcs5params *pkcs5)
{
    int i;
    Qc3_Format_ALGD0100_T hctx;
    int len = pkcs5->saltlen;
    char *data = (char *) pkcs5->salt;
    Qus_EC_t errcode;

    *dk = NULL;
    if(pkcs5->dklen > pkcs5->hashlen)
        return -1;

    /* Allocate the derived key buffer. */
    *dk = LIBSSH2_ALLOC(session, pkcs5->hashlen);
    if(!*dk)
        return -1;

    set_EC_length(errcode, sizeof(errcode));
    errcode.Bytes_Available = 1;   /* Defaults to error flagging. */

    /* Initial hash. */
    if(_libssh2_os400qc3_hash_init(&hctx, pkcs5->hash)) {
        if(_libssh2_os400qc3_hash_update(&hctx,
                                         passphrase, strlen(passphrase))) {
            hctx.Final_Op_Flag = Qc3_Final;
            Qc3CalculateHash((char *) pkcs5->salt, &len, Qc3_Data,
                             (char *) &hctx, Qc3_Alg_Token, anycsp, NULL, *dk,
                             (char *) &errcode);

            /* Iterate. */
            len = pkcs5->hashlen;
            for(i = 1; !errcode.Bytes_Available && i < pkcs5->itercount; i++)
                Qc3CalculateHash((char *) *dk, &len, Qc3_Data, (char *) &hctx,
                                 Qc3_Alg_Token, anycsp, NULL, *dk,
                                 (char *) &errcode);
        }

        Qc3DestroyAlgorithmContext(hctx.Alg_Context_Token, (char *) &ecnull);
    }

    if(errcode.Bytes_Available) {
        LIBSSH2_FREE(session, *dk);
        *dk = NULL;
        return -1;
    }

    /* Special stuff for PBES1: split derived key into 8-byte key and 8-byte
       initialization vector. */
    pkcs5->dklen = 8;
    pkcs5->ivlen = 8;
    pkcs5->iv = *dk + 8;

    return 0;
}

static int
pbkdf2(LIBSSH2_SESSION *session, char **dk, const unsigned char *passphrase,
       pkcs5params *pkcs5)
{
    size_t i;
    size_t k;
    int j;
    int l;
    uint32_t ni;
    unsigned long long t;
    char *mac;
    char *buf;
    _libssh2_os400qc3_crypto_ctx hctx;

    *dk = NULL;
    t = ((unsigned long long) pkcs5->dklen + pkcs5->hashlen - 1) /
        pkcs5->hashlen;
    if(t > 0xFFFFFFFF)
        return -1;
    mac = alloca(pkcs5->hashlen);
    if(!mac)
        return -1;

    /* Create an HMAC context for our computations. */
    if(!libssh2_os400qc3_hmac_init(&hctx, pkcs5->hash, pkcs5->hashlen,
                                   (void *) passphrase, strlen(passphrase)))
        return -1;

    /* Allocate the derived key buffer. */
    l = t;
    buf = LIBSSH2_ALLOC(session, l * pkcs5->hashlen);
    if(!buf)
        return -1;
    *dk = buf;

    /* Process each hLen-size blocks. */
    for(i = 1; i <= l; i++) {
        ni = htonl(i);
        if(!_libssh2_hmac_update(&hctx, pkcs5->salt, pkcs5->saltlen) ||
           !_libssh2_hmac_update(&hctx, &ni, sizeof(ni)) ||
           !_libssh2_hmac_final(&hctx, mac)) {
            LIBSSH2_FREE(session, buf);
            *dk = NULL;
            _libssh2_os400qc3_crypto_dtor(&hctx);
            return -1;
        }
        memcpy(buf, mac, pkcs5->hashlen);
        for(j = 1; j < pkcs5->itercount; j++) {
            if(!_libssh2_hmac_update(&hctx, mac, pkcs5->hashlen) ||
               !_libssh2_hmac_final(&hctx, mac)) {
                LIBSSH2_FREE(session, buf);
                *dk = NULL;
                _libssh2_os400qc3_crypto_dtor(&hctx);
                return -1;
            }
            for(k = 0; k < pkcs5->hashlen; k++)
                buf[k] ^= mac[k];
        }
        buf += pkcs5->hashlen;
    }

    /* Computation done. Release HMAC context. */
    _libssh2_os400qc3_crypto_dtor(&hctx);
    return 0;
}

static int
parse_pkcs5_algorithm(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                      asn1Element *algid, pkcs5algo **algotable)
{
    asn1Element oid;
    asn1Element param;
    char *cp;

    cp = getASN1Element(&oid, algid->beg, algid->end);
    if(!cp || *oid.header != ASN1_OBJ_ID)
        return -1;
    param.header = NULL;
    if(cp < algid->end)
        cp = getASN1Element(&param, cp, algid->end);
    if(cp != algid->end)
        return -1;
    for(; *algotable; algotable++)
        if(!oidcmp(&oid, (*algotable)->oid))
            return (*(*algotable)->parse)(session, pkcs5, *algotable,
                                          param.header ? &param : NULL);
    return -1;
}

static int
parse_pbes2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
            pkcs5algo *algo, asn1Element *param)
{
    asn1Element keyDerivationFunc;
    asn1Element encryptionScheme;
    char *cp;

    if(!param || *param->header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    cp = getASN1Element(&keyDerivationFunc, param->beg, param->end);
    if(!cp || *keyDerivationFunc.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    if(getASN1Element(&encryptionScheme, cp, param->end) != param->end ||
        *encryptionScheme.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    if(parse_pkcs5_algorithm(session, pkcs5, &encryptionScheme, pbes2enctable))
        return -1;
    if(parse_pkcs5_algorithm(session, pkcs5, &keyDerivationFunc, pbkdf2table))
        return -1;
    return 0;
}

static int
parse_pbkdf2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
             pkcs5algo *algo, asn1Element *param)
{
    asn1Element salt;
    asn1Element iterationCount;
    asn1Element keyLength;
    asn1Element prf;
    unsigned long itercount;
    char *cp;

    if(!param || *param->header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    cp = getASN1Element(&salt, param->beg, param->end);
    /* otherSource not supported. */
    if(!cp || *salt.header != ASN1_OCTET_STRING)
        return -1;
    cp = getASN1Element(&iterationCount, cp, param->end);
    if(!cp || *iterationCount.header != ASN1_INTEGER)
        return -1;
    keyLength.header = prf.header = NULL;
    if(cp < param->end) {
        cp = getASN1Element(&prf, cp, param->end);
        if(!cp)
            return -1;
        if(*prf.header == ASN1_INTEGER) {
            keyLength = prf;
            prf.header = NULL;
            if(cp < param->end)
                cp = getASN1Element(&prf, cp, param->end);
        }
        if(cp != param->end)
            return -1;
    }
    pkcs5->hash = algo->hash;
    pkcs5->hashlen = algo->hashlen;
    if(prf.header) {
        if(*prf.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
            return -1;
        if(parse_pkcs5_algorithm(session, pkcs5, &prf, kdf2prftable))
            return -1;
    }
    pkcs5->saltlen = salt.end - salt.beg;
    pkcs5->salt = salt.beg;
    if(asn1getword(&iterationCount, &itercount) ||
        !itercount || itercount > 100000)
        return -1;
    pkcs5->itercount = itercount;
    pkcs5->kdf = pbkdf2;
    return 0;
}

static int
parse_hmacWithSHA1(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
                   pkcs5algo *algo, asn1Element *param)
{
    if(!param || *param->header != ASN1_NULL)
        return -1;
    pkcs5->hash = algo->hash;
    pkcs5->hashlen = algo->hashlen;
    return 0;
}

static int
parse_iv(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
         pkcs5algo *algo, asn1Element *param)
{
    if(!param || *param->header != ASN1_OCTET_STRING ||
        param->end - param->beg != algo->ivlen)
        return -1;
    pkcs5->cipher = algo->cipher;
    pkcs5->blocksize = algo->blocksize;
    pkcs5->mode = algo->mode;
    pkcs5->padopt = algo->padopt;
    pkcs5->padchar = algo->padchar;
    pkcs5->dklen = algo->keylen;
    pkcs5->ivlen = algo->ivlen;
    pkcs5->iv = param->beg;
    return 0;
}

static int
parse_rc2(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
          pkcs5algo *algo, asn1Element *param)
{
    asn1Element iv;
    unsigned long effkeysize;
    char *cp;

    if(!param || *param->header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    cp = getASN1Element(&iv, param->beg, param->end);
    if(!cp)
        return -1;
    effkeysize = algo->effkeysize;
    if(*iv.header == ASN1_INTEGER) {
        if(asn1getword(&iv, &effkeysize) || effkeysize > 1024)
            return -1;

        cp = getASN1Element(&iv, cp, param->end);
        if(effkeysize < 256)
            switch(effkeysize) {
            case 160:
                effkeysize = 40;
            case 120:
                effkeysize = 64;
            case 58:
                effkeysize = 128;
                break;
            default:
                return -1;
            }
    }
    if(effkeysize > 1024 || cp != param->end ||
        *iv.header != ASN1_OCTET_STRING || iv.end - iv.beg != algo->ivlen)
        return -1;
    pkcs5->cipher = algo->cipher;
    pkcs5->blocksize = algo->blocksize;
    pkcs5->mode = algo->mode;
    pkcs5->padopt = algo->padopt;
    pkcs5->padchar = algo->padchar;
    pkcs5->ivlen = algo->ivlen;
    pkcs5->iv = iv.beg;
    pkcs5->effkeysize = effkeysize;
    pkcs5->dklen = (effkeysize + 8 - 1) / 8;
    return 0;
}

static int
parse_pbes1(LIBSSH2_SESSION *session, pkcs5params *pkcs5,
            pkcs5algo *algo, asn1Element *param)
{
    asn1Element salt;
    asn1Element iterationCount;
    unsigned long itercount;
    char *cp;

    if(!param || *param->header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;

    cp = getASN1Element(&salt, param->beg, param->end);
    if(!cp || *salt.header != ASN1_OCTET_STRING ||
        salt.end - salt.beg != algo->saltlen)
        return -1;
    if(getASN1Element(&iterationCount, cp, param->end) != param->end ||
        *iterationCount.header != ASN1_INTEGER)
        return -1;
    if(asn1getword(&iterationCount, &itercount) ||
        !itercount || itercount > 100000)
        return -1;
    pkcs5->cipher = algo->cipher;
    pkcs5->blocksize = algo->blocksize;
    pkcs5->mode = algo->mode;
    pkcs5->padopt = algo->padopt;
    pkcs5->padchar = algo->padchar;
    pkcs5->hash = algo->hash;
    pkcs5->hashlen = algo->hashlen;
    pkcs5->dklen = 16;
    pkcs5->saltlen = algo->saltlen;
    pkcs5->effkeysize = algo->effkeysize;
    pkcs5->salt = salt.beg;
    pkcs5->kdf = pbkdf1;
    pkcs5->itercount = itercount;
    return 0;
}

static int
pkcs8kek(LIBSSH2_SESSION *session, _libssh2_os400qc3_crypto_ctx **ctx,
         const unsigned char *data, unsigned int datalen,
         const unsigned char *passphrase, asn1Element *privkeyinfo)
{
    asn1Element encprivkeyinfo;
    asn1Element pkcs5alg;
    pkcs5params pkcs5;
    size_t pplen;
    char *cp;
    unsigned long t;
    int i;
    char *dk = NULL;
    Qc3_Format_ALGD0200_T algd;
    Qus_EC_t errcode;

    /* Determine if the PKCS#8 data is encrypted and, if so, set-up a
       key encryption key and algorithm in context.
       Return 1 if encrypted, 0, if not, -1 if error. */

    *ctx = NULL;
    privkeyinfo->beg = (char *) data;
    privkeyinfo->end = privkeyinfo->beg + datalen;

    /* If no passphrase is given, it cannot be an encrypted key. */
    if(!passphrase || !*passphrase)
        return 0;

    /* Parse PKCS#8 data, checking if ASN.1 format is PrivateKeyInfo or
       EncryptedPrivateKeyInfo. */
    if(getASN1Element(&encprivkeyinfo, privkeyinfo->beg, privkeyinfo->end) !=
        (char *) data + datalen ||
        *encprivkeyinfo.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    cp = getASN1Element(&pkcs5alg, encprivkeyinfo.beg, encprivkeyinfo.end);
    if(!cp)
        return -1;

    switch(*pkcs5alg.header) {
    case ASN1_INTEGER:                          /* Version. */
        return 0;       /* This is a PrivateKeyInfo --> not encrypted. */
    case ASN1_SEQ | ASN1_CONSTRUCTED:           /* AlgorithIdentifier. */
        break;          /* This is an EncryptedPrivateKeyInfo --> encrypted. */
    default:
        return -1;      /* Unrecognized: error. */
    }

    /* Get the encrypted key data. */
    if(getASN1Element(privkeyinfo, cp, encprivkeyinfo.end) !=
        encprivkeyinfo.end || *privkeyinfo->header != ASN1_OCTET_STRING)
        return -1;

    /* PKCS#5: parse the PBES AlgorithmIdentifier and recursively get all
       encryption parameters. */
    memset((char *) &pkcs5, 0, sizeof(pkcs5));
    if(parse_pkcs5_algorithm(session, &pkcs5, &pkcs5alg, pbestable))
        return -1;

    /* Compute the derived key. */
    if((*pkcs5.kdf)(session, &dk, passphrase, &pkcs5))
        return -1;

    /* Prepare the algorithm descriptor. */
    memset((char *) &algd, 0, sizeof(algd));
    algd.Block_Cipher_Alg = pkcs5.cipher;
    algd.Block_Length = pkcs5.blocksize;
    algd.Mode = pkcs5.mode;
    algd.Pad_Option = pkcs5.padopt;
    algd.Pad_Character = pkcs5.padchar;
    algd.Effective_Key_Size = pkcs5.effkeysize;
    memcpy(algd.Init_Vector, pkcs5.iv, pkcs5.ivlen);

    /* Create the key and algorithm context tokens. */
    *ctx = libssh2_init_crypto_ctx(NULL);
    if(!*ctx) {
        LIBSSH2_FREE(session, dk);
        return -1;
    }
    libssh2_init_crypto_ctx(*ctx);
    set_EC_length(errcode, sizeof(errcode));
    Qc3CreateKeyContext(dk, &pkcs5.dklen, binstring, &algd.Block_Cipher_Alg,
                        qc3clear, NULL, NULL, (*ctx)->key.Key_Context_Token,
                        (char *) &errcode);
    LIBSSH2_FREE(session, dk);
    if(errcode.Bytes_Available) {
        free((char *) *ctx);
        *ctx = NULL;
        return -1;
    }

    Qc3CreateAlgorithmContext((char *) &algd, Qc3_Alg_Block_Cipher,
                              (*ctx)->hash.Alg_Context_Token, &errcode);
    if(errcode.Bytes_Available) {
        Qc3DestroyKeyContext((*ctx)->key.Key_Context_Token, (char *) &ecnull);
        free((char *) *ctx);
        *ctx = NULL;
        return -1;
    }
    return 1;       /* Tell it's encrypted. */
}

static int
rsapkcs8privkey(LIBSSH2_SESSION *session,
                const unsigned char *data, unsigned int datalen,
                const unsigned char *passphrase, void *loadkeydata)
{
    libssh2_rsa_ctx *ctx = (libssh2_rsa_ctx *) loadkeydata;
    char keyform = Qc3_Clear;
    char *kek = NULL;
    char *kea = NULL;
    _libssh2_os400qc3_crypto_ctx *kekctx;
    asn1Element pki;
    int pkilen;
    Qus_EC_t errcode;

    switch(pkcs8kek(session, &kekctx, data, datalen, passphrase, &pki)) {
    case 1:
        keyform = Qc3_Encrypted;
        kek = kekctx->key.Key_Context_Token;
        kea = kekctx->hash.Alg_Context_Token;
    case 0:
        break;
    default:
        return -1;
    }

    set_EC_length(errcode, sizeof(errcode));
    pkilen = pki.end - pki.beg;
    Qc3CreateKeyContext((unsigned char *) pki.beg, &pkilen, berstring,
                        rsaprivate, &keyform, kek, kea,
                        ctx->key.Key_Context_Token, (char *) &errcode);
    if(errcode.Bytes_Available) {
        if(kekctx)
            _libssh2_os400qc3_crypto_dtor(kekctx);
        return -1;
    }
    ctx->kek = kekctx;
    return 0;
}

static char *
storewithlength(char *p, const char *data, int length)
{
    _libssh2_htonu32(p, length);
    if(length)
        memcpy(p + 4, data, length);
    return p + 4 + length;
}

static int
sshrsapubkey(LIBSSH2_SESSION *session, char **sshpubkey,
             asn1Element *params, asn1Element *key, const char *method)
{
    int methlen = strlen(method);
    asn1Element keyseq;
    asn1Element m;
    asn1Element e;
    int len;
    char *cp;

    if(getASN1Element(&keyseq, key->beg + 1, key->end) != key->end ||
        *keyseq.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    if(!getASN1Element(&m, keyseq.beg, keyseq.end) ||
        *m.header != ASN1_INTEGER)
        return -1;
    if(getASN1Element(&e, m.end, keyseq.end) != keyseq.end ||
        *e.header != ASN1_INTEGER)
        return -1;
    len = 4 + methlen + 4 + (e.end - e.beg) + 4 + (m.end - m.beg);
    cp = LIBSSH2_ALLOC(session, len);
    if(!cp)
        return -1;
    *sshpubkey = cp;
    cp = storewithlength(cp, method, methlen);
    cp = storewithlength(cp, e.beg, e.end - e.beg);
    cp = storewithlength(cp, m.beg, m.end - m.beg);
    return len;
}

static int
rsapkcs8pubkey(LIBSSH2_SESSION *session,
               const unsigned char *data, unsigned int datalen,
               const unsigned char *passphrase, void *loadkeydata)
{
    loadpubkeydata *p = (loadpubkeydata *) loadkeydata;
    char *buf;
    int len;
    char *cp;
    int i;
    char keyform = Qc3_Clear;
    char *kek = NULL;
    char *kea = NULL;
    _libssh2_os400qc3_crypto_ctx *kekctx;
    asn1Element subjpubkeyinfo;
    asn1Element algorithmid;
    asn1Element algorithm;
    asn1Element subjpubkey;
    asn1Element parameters;
    asn1Element pki;
    int pkilen;
    Qus_EC_t errcode;

    buf = alloca(datalen);
    if(!buf)
        return -1;

    switch(pkcs8kek(session, &kekctx, data, datalen, passphrase, &pki)) {
    case 1:
        keyform = Qc3_Encrypted;
        kek = kekctx->key.Key_Context_Token;
        kea = kekctx->hash.Alg_Context_Token;
    case 0:
        break;
    default:
        return -1;
    }

    set_EC_length(errcode, sizeof(errcode));
    pkilen = pki.end - pki.beg;
    Qc3ExtractPublicKey(pki.beg, &pkilen, berstring, &keyform,
                        kek, kea, buf, (int *) &datalen, &len, &errcode);
    _libssh2_os400qc3_crypto_dtor(kekctx);
    if(errcode.Bytes_Available)
        return -1;
    /* Get the algorithm OID and key data from SubjectPublicKeyInfo. */
    if(getASN1Element(&subjpubkeyinfo, buf, buf + len) != buf + len ||
        *subjpubkeyinfo.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    cp = getASN1Element(&algorithmid, subjpubkeyinfo.beg, subjpubkeyinfo.end);
    if(!cp || *algorithmid.header != (ASN1_SEQ | ASN1_CONSTRUCTED))
        return -1;
    if(!getASN1Element(&algorithm, algorithmid.beg, algorithmid.end) ||
        *algorithm.header != ASN1_OBJ_ID)
        return -1;
    if(getASN1Element(&subjpubkey, cp, subjpubkeyinfo.end) !=
        subjpubkeyinfo.end || *subjpubkey.header != ASN1_BIT_STRING)
        return -1;
    /* Check for supported algorithm. */
    for(i = 0; pka[i].oid; i++)
        if(!oidcmp(&algorithm, pka[i].oid)) {
            len = (*pka[i].sshpubkey)(session, &p->data, &algorithmid,
                                      &subjpubkey, pka[i].method);
            if(len < 0)
                return -1;
            p->length = len;
            p->method = pka[i].method;
            return 0;
        }
    return -1;                              /* Algorithm not supported. */
}

static int
pkcs1topkcs8(LIBSSH2_SESSION *session,
             const unsigned char **data8, unsigned int *datalen8,
             const unsigned char *data1, unsigned int datalen1)
{
    asn1Element *prvk;
    asn1Element *pkcs8;
    unsigned char *data;

    *data8 = NULL;
    *datalen8 = 0;
    if(datalen1 < 2)
        return -1;
    prvk = asn1_new_from_bytes(data1, datalen1);
    if(!prvk)
        return -1;
    pkcs8 = rsaprivatekeyinfo(prvk);
    asn1delete(prvk);
    if(!prvk) {
        asn1delete(pkcs8);
        pkcs8 = NULL;
    }
    if(!pkcs8)
        return -1;
    data = (unsigned char *) LIBSSH2_ALLOC(session,
                                           pkcs8->end - pkcs8->header);
    if(!data) {
        asn1delete(pkcs8);
        return -1;
    }
    *data8 = data;
    *datalen8 = pkcs8->end - pkcs8->header;
    memcpy((char *) data, (char *) pkcs8->header, *datalen8);
    asn1delete(pkcs8);
    return 0;
}

static int
rsapkcs1privkey(LIBSSH2_SESSION *session,
                const unsigned char *data, unsigned int datalen,
                const unsigned char *passphrase, void *loadkeydata)
{
    const unsigned char *data8;
    unsigned int datalen8;
    int ret;

    if(pkcs1topkcs8(session, &data8, &datalen8, data, datalen))
        return -1;
    ret = rsapkcs8privkey(session, data8, datalen8, passphrase, loadkeydata);
    LIBSSH2_FREE(session, (char *) data8);
    return ret;
}

static int
rsapkcs1pubkey(LIBSSH2_SESSION *session,
               const unsigned char *data, unsigned int datalen,
               const unsigned char *passphrase, void *loadkeydata)
{
    const unsigned char *data8;
    unsigned int datalen8;
    int ret;

    if(pkcs1topkcs8(session, &data8, &datalen8, data, datalen))
        return -1;
    ret = rsapkcs8pubkey(session, data8, datalen8, passphrase, loadkeydata);
    LIBSSH2_FREE(session, (char *) data8);
    return ret;
}

static int
try_pem_load(LIBSSH2_SESSION *session, FILE *fp,
             const unsigned char *passphrase,
             const char *header, const char *trailer,
             loadkeyproc proc, void *loadkeydata)
{
    unsigned char *data = NULL;
    size_t datalen = 0;
    int c;
    int ret;

    fseek(fp, 0L, SEEK_SET);
    for(;;) {
        ret = _libssh2_pem_parse(session, header, trailer,
                                 passphrase,
                                 fp, &data, &datalen);

        if(!ret) {
            ret = (*proc)(session, data, datalen, passphrase, loadkeydata);
            if(!ret)
                return 0;
        }

        if(data) {
            LIBSSH2_FREE(session, data);
            data = NULL;
        }
        c = getc(fp);

        if(c == EOF)
            break;

        ungetc(c, fp);
    }

    return -1;
}

static int
load_rsa_private_file(LIBSSH2_SESSION *session, const char *filename,
                      unsigned const char *passphrase,
                      loadkeyproc proc1, loadkeyproc proc8, void *loadkeydata)
{
    FILE *fp = fopen(filename, fopenrmode);
    unsigned char *data = NULL;
    size_t datalen = 0;
    int ret;
    long filesize;

    if(!fp)
        return -1;

    /* Try with "ENCRYPTED PRIVATE KEY" PEM armor.
       --> PKCS#8 EncryptedPrivateKeyInfo */
    ret = try_pem_load(session, fp, passphrase, beginencprivkeyhdr,
                       endencprivkeyhdr, proc8, loadkeydata);

    /* Try with "PRIVATE KEY" PEM armor.
       --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
    if(ret)
        ret = try_pem_load(session, fp, passphrase, beginprivkeyhdr,
                           endprivkeyhdr, proc8, loadkeydata);

    /* Try with "RSA PRIVATE KEY" PEM armor.
       --> PKCS#1 RSAPrivateKey */
    if(ret)
        ret = try_pem_load(session, fp, passphrase, beginrsaprivkeyhdr,
                           endrsaprivkeyhdr, proc1, loadkeydata);
    fclose(fp);

    if(ret) {
        /* Try DER encoding. */
        fp = fopen(filename, fopenrbmode);
        fseek(fp, 0L, SEEK_END);
        filesize = ftell(fp);

        if(filesize <= 32768) {        /* Limit to a reasonable size. */
            datalen = filesize;
            data = (unsigned char *) alloca(datalen);
            if(data) {
                fseek(fp, 0L, SEEK_SET);
                fread(data, datalen, 1, fp);

                /* Try as PKCS#8 DER data.
                   --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
                ret = (*proc8)(session, data, datalen, passphrase,
                               loadkeydata);

                /* Try as PKCS#1 DER data.
                   --> PKCS#1 RSAPrivateKey */
                if(ret)
                    ret = (*proc1)(session, data, datalen, passphrase,
                                   loadkeydata);
            }
        }
        fclose(fp);
    }

    return ret;
}

int
_libssh2_rsa_new_private(libssh2_rsa_ctx **rsa, LIBSSH2_SESSION *session,
                         const char *filename, unsigned const char *passphrase)
{
    libssh2_rsa_ctx *ctx = libssh2_init_crypto_ctx(NULL);
    int ret;

    if(!ctx)
        return -1;
    ret = load_rsa_private_file(session, filename, passphrase,
                                rsapkcs1privkey, rsapkcs8privkey,
                                (void *) ctx);
    if(ret) {
        _libssh2_os400qc3_crypto_dtor(ctx);
        ctx = NULL;
    }
    *rsa = ctx;
    return ret;
}

int
_libssh2_pub_priv_keyfile(LIBSSH2_SESSION *session,
                          unsigned char **method, size_t *method_len,
                          unsigned char **pubkeydata, size_t *pubkeydata_len,
                          const char *privatekey, const char *passphrase)
{
    loadpubkeydata p;
    int ret;

    *method = NULL;
    *method_len = 0;
    *pubkeydata = NULL;
    *pubkeydata_len = 0;

    ret = load_rsa_private_file(session, privatekey, passphrase,
                                rsapkcs1pubkey, rsapkcs8pubkey, (void *) &p);
    if(!ret) {
        *method_len = strlen(p.method);
        *method = LIBSSH2_ALLOC(session, *method_len);
        if(*method)
            memcpy((char *) *method, p.method, *method_len);
        else
            ret = -1;
    }

    if(ret) {
        if(*method)
            LIBSSH2_FREE(session, *method);
        if(p.data)
            LIBSSH2_FREE(session, (void *) p.data);
        *method = NULL;
        *method_len = 0;
    }
    else {
        *pubkeydata = (unsigned char *) p.data;
        *pubkeydata_len = p.length;
    }

    return ret;
}

int
_libssh2_rsa_new_private_frommemory(libssh2_rsa_ctx **rsa,
                                    LIBSSH2_SESSION *session,
                                    const char *filedata,
                                    size_t filedata_len,
                                    unsigned const char *passphrase)
{
    libssh2_rsa_ctx *ctx = libssh2_init_crypto_ctx(NULL);
    unsigned char *data = NULL;
    size_t datalen = 0;
    int ret;

    if(!ctx)
        return -1;

    /* Try with "ENCRYPTED PRIVATE KEY" PEM armor.
       --> PKCS#8 EncryptedPrivateKeyInfo */
    ret = _libssh2_pem_parse_memory(session,
                                    beginencprivkeyhdr, endencprivkeyhdr,
                                    filedata, filedata_len, &data, &datalen);

    /* Try with "PRIVATE KEY" PEM armor.
       --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
    if(ret)
        ret = _libssh2_pem_parse_memory(session,
                                        beginprivkeyhdr, endprivkeyhdr,
                                        filedata, filedata_len,
                                        &data, &datalen);

    if(!ret) {
        /* Process PKCS#8. */
        ret = rsapkcs8privkey(session,
                              data, datalen, passphrase, (void *) &ctx);
    }
    else {
        /* Try with "RSA PRIVATE KEY" PEM armor.
           --> PKCS#1 RSAPrivateKey */
        ret = _libssh2_pem_parse_memory(session,
                                        beginrsaprivkeyhdr, endrsaprivkeyhdr,
                                        filedata, filedata_len,
                                        &data, &datalen);
        if(!ret)
            ret = rsapkcs1privkey(session,
                                  data, datalen, passphrase, (void *) &ctx);
    }

    if(ret) {
        /* Try as PKCS#8 DER data.
           --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
        ret = rsapkcs8privkey(session, filedata, filedata_len,
                              passphrase, (void *) &ctx);

        /* Try as PKCS#1 DER data.
           --> PKCS#1 RSAPrivateKey */
        if(ret)
            ret = rsapkcs1privkey(session, filedata, filedata_len,
                                  passphrase, (void *) &ctx);
    }

    if(data)
        LIBSSH2_FREE(session, data);

    if(ret) {
        _libssh2_os400qc3_crypto_dtor(ctx);
        ctx = NULL;
    }

    *rsa = ctx;
    return ret;
}

int
_libssh2_pub_priv_keyfilememory(LIBSSH2_SESSION *session,
                                unsigned char **method, size_t *method_len,
                                unsigned char **pubkeydata,
                                size_t *pubkeydata_len,
                                const char *privatekeydata,
                                size_t privatekeydata_len,
                                const char *passphrase)
{
    loadpubkeydata p;
    unsigned char *data = NULL;
    size_t datalen = 0;
    const char *meth;
    int ret;

    *method = NULL;
    *method_len = 0;
    *pubkeydata = NULL;
    *pubkeydata_len = 0;

    /* Try with "ENCRYPTED PRIVATE KEY" PEM armor.
       --> PKCS#8 EncryptedPrivateKeyInfo */
    ret = _libssh2_pem_parse_memory(session,
                                    beginencprivkeyhdr, endencprivkeyhdr,
                                    privatekeydata, privatekeydata_len,
                                    &data, &datalen);

    /* Try with "PRIVATE KEY" PEM armor.
       --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
    if(ret)
        ret = _libssh2_pem_parse_memory(session,
                                        beginprivkeyhdr, endprivkeyhdr,
                                        privatekeydata, privatekeydata_len,
                                        &data, &datalen);

    if(!ret) {
        /* Process PKCS#8. */
        ret = rsapkcs8pubkey(session,
                             data, datalen, passphrase, (void *) &p);
    }
    else {
        /* Try with "RSA PRIVATE KEY" PEM armor.
           --> PKCS#1 RSAPrivateKey */
        ret = _libssh2_pem_parse_memory(session,
                                        beginrsaprivkeyhdr, endrsaprivkeyhdr,
                                        privatekeydata, privatekeydata_len,
                                        &data, &datalen);
        if(!ret)
            ret = rsapkcs1pubkey(session,
                                 data, datalen, passphrase, (void *) &p);
    }

    if(ret) {
        /* Try as PKCS#8 DER data.
           --> PKCS#8 PrivateKeyInfo or EncryptedPrivateKeyInfo */
        ret = rsapkcs8pubkey(session, privatekeydata, privatekeydata_len,
                             passphrase, (void *) &p);

        /* Try as PKCS#1 DER data.
           --> PKCS#1 RSAPrivateKey */
        if(ret)
            ret = rsapkcs1pubkey(session, privatekeydata, privatekeydata_len,
                                 passphrase, (void *) &p);
    }

    if(data)
        LIBSSH2_FREE(session, data);

    if(!ret) {
        *method_len = strlen(p.method);
        *method = LIBSSH2_ALLOC(session, *method_len);
        if(*method)
            memcpy((char *) *method, p.method, *method_len);
        else
            ret = -1;
    }
    if(ret) {
        if(*method)
            LIBSSH2_FREE(session, *method);
        if(p.data)
            LIBSSH2_FREE(session, (void *) p.data);
        *method = NULL;
        *method_len = 0;
    }
    else {
        *pubkeydata = (unsigned char *) p.data;
        *pubkeydata_len = p.length;
    }

    return ret;
}

int
_libssh2_sk_pub_keyfilememory(LIBSSH2_SESSION *session,
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
    return _libssh2_error(session, LIBSSH2_ERROR_FILE,
                    "Unable to extract public SK key from private key file: "
                    "Method unimplemented in OS/400 QC3 backend");
}

int
_libssh2_rsa_sha2_verify(libssh2_rsa_ctx *rsa, size_t hash_len,
                         const unsigned char *sig, size_t sig_len,
                         const unsigned char *m, size_t m_len)
{
    Qus_EC_t errcode;
    Qc3_Format_ALGD0400_T algd;
    int slen = (int)sig_len;
    int mlen = (int)m_len;

    memset(&algd, 0, sizeof(algd));
    algd.Public_Key_Alg = Qc3_RSA;
    algd.PKA_Block_Format = Qc3_PKCS1_01;
    switch(hash_len) {
    case SHA_DIGEST_LENGTH:
        algd.Signing_Hash_Alg = Qc3_SHA1;
        break;
    case SHA256_DIGEST_LENGTH:
        algd.Signing_Hash_Alg = Qc3_SHA256;
        break;
    case SHA512_DIGEST_LENGTH:
        algd.Signing_Hash_Alg = Qc3_SHA512;
        break;
    default:
        return -1;
    }

    set_EC_length(errcode, sizeof(errcode));
    Qc3VerifySignature((char *) sig, &slen, (char *) m, &mlen, Qc3_Data,
                       (char *) &algd, Qc3_Alg_Public_Key,
                       (char *) &rsa->key, Qc3_Key_Token, anycsp,
                       NULL, (char *) &errcode);
    return errcode.Bytes_Available ? -1 : 0;
}

int
_libssh2_rsa_sha1_verify(libssh2_rsa_ctx *rsa,
                         const unsigned char *sig, size_t sig_len,
                         const unsigned char *m, size_t m_len)
{
    return _libssh2_rsa_sha2_verify(rsa, SHA_DIGEST_LENGTH,
                                    sig, sig_len, m, m_len);
}

int
_libssh2_os400qc3_rsa_signv(LIBSSH2_SESSION *session,
                            int algo,
                            unsigned char **signature,
                            size_t *signature_len,
                            int veccount,
                            const struct iovec vector[],
                            libssh2_rsa_ctx *ctx)
{
    Qus_EC_t errcode;
    Qc3_Format_ALGD0400_T algd;
    int siglen;
    unsigned char *sig;
    char sigbuf[8192];
    int sigbufsize = sizeof(sigbuf);

    algd.Public_Key_Alg = Qc3_RSA;
    algd.PKA_Block_Format = Qc3_PKCS1_01;
    memset(algd.Reserved, 0, sizeof(algd.Reserved));
    algd.Signing_Hash_Alg = algo;
    set_EC_length(errcode, sizeof(errcode));
    Qc3CalculateSignature((char *) vector, &veccount, Qc3_Array,
                          (char *) &algd, Qc3_Alg_Public_Key,
                          (char *) &ctx->key, Qc3_Key_Token,
                          anycsp, NULL, sigbuf, &sigbufsize, &siglen,
                          (char *) &errcode);
    if(errcode.Bytes_Available)
        return -1;
    sig = LIBSSH2_ALLOC(session, siglen);
    if(!sig)
        return -1;
    memcpy((char *) sig, sigbuf, siglen);
    *signature = sig;
    *signature_len = siglen;
    return 0;
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

    if(key_method_len == 7 &&
       memcmp(key_method, "ssh-rsa", key_method_len) == 0) {
        return "rsa-sha2-512,rsa-sha2-256"
#if LIBSSH2_RSA_SHA1
            ",ssh-rsa"
#endif
            ;
    }

    return NULL;
}

#endif /* LIBSSH2_CRYPTO_C */

/* vim: set expandtab ts=4 sw=4: */
