/* Copyright (C) The Written Word, Inc.
 * Copyright (C) Simon Josefsson
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

static int
readline(char *line, int line_size, FILE * fp)
{
    size_t len;

    if(!line) {
        return -1;
    }
    if(!fgets(line, line_size, fp)) {
        return -1;
    }

    if(*line) {
        len = strlen(line);
        if(len > 0 && line[len - 1] == '\n') {
            line[len - 1] = '\0';
        }
    }

    if(*line) {
        len = strlen(line);
        if(len > 0 && line[len - 1] == '\r') {
            line[len - 1] = '\0';
        }
    }

    return 0;
}

static int
readline_memory(char *line, size_t line_size,
                const char *filedata, size_t filedata_len,
                size_t *filedata_offset)
{
    size_t off, len;

    off = *filedata_offset;

    for(len = 0; off + len < filedata_len && len < line_size - 1; len++) {
        if(filedata[off + len] == '\n' ||
            filedata[off + len] == '\r') {
                break;
        }
    }

    if(len) {
        memcpy(line, filedata + off, len);
        *filedata_offset += len;
    }

    line[len] = '\0';
    *filedata_offset += 1;

    return 0;
}

#define LINE_SIZE 128

static const char *crypt_annotation = "Proc-Type: 4,ENCRYPTED";

static unsigned char hex_decode(char digit)
{
    return (unsigned char)
        ((digit >= 'A') ? (0xA + (digit - 'A')) : (digit - '0'));
}

int
_libssh2_pem_parse(LIBSSH2_SESSION * session,
                   const char *headerbegin,
                   const char *headerend,
                   const unsigned char *passphrase,
                   FILE * fp, unsigned char **data, size_t *datalen)
{
    char line[LINE_SIZE];
    unsigned char iv[LINE_SIZE];
    char *b64data = NULL;
    size_t b64datalen = 0;
    int ret;
    const LIBSSH2_CRYPT_METHOD *method = NULL;

    do {
        *line = '\0';

        if(readline(line, LINE_SIZE, fp)) {
            return -1;
        }
    } while(strcmp(line, headerbegin) != 0);

    if(readline(line, LINE_SIZE, fp)) {
        return -1;
    }

    if(passphrase &&
            memcmp(line, crypt_annotation, strlen(crypt_annotation)) == 0) {
        const LIBSSH2_CRYPT_METHOD **all_methods, *cur_method;
        int i;

        if(readline(line, LINE_SIZE, fp)) {
            ret = -1;
            goto out;
        }

        all_methods = libssh2_crypt_methods();
        /* !checksrc! disable EQUALSNULL 1 */
        while((cur_method = *all_methods++) != NULL) {
            if(*cur_method->pem_annotation &&
                    memcmp(line, cur_method->pem_annotation,
                           strlen(cur_method->pem_annotation)) == 0) {
                method = cur_method;
                memcpy(iv, line + strlen(method->pem_annotation) + 1,
                       2*method->iv_len);
            }
        }

        /* None of the available crypt methods were able to decrypt the key */
        if(!method)
            return -1;

        /* Decode IV from hex */
        for(i = 0; i < method->iv_len; ++i) {
            iv[i]  = (unsigned char)(hex_decode(iv[2*i]) << 4);
            iv[i] |= hex_decode(iv[2*i + 1]);
        }

        /* skip to the next line */
        if(readline(line, LINE_SIZE, fp)) {
            ret = -1;
            goto out;
        }
    }

    do {
        if(*line) {
            char *tmp;
            size_t linelen;

            linelen = strlen(line);
            tmp = LIBSSH2_REALLOC(session, b64data, b64datalen + linelen);
            if(!tmp) {
                _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                               "Unable to allocate memory for PEM parsing");
                ret = -1;
                goto out;
            }
            memcpy(tmp + b64datalen, line, linelen);
            b64data = tmp;
            b64datalen += linelen;
        }

        *line = '\0';

        if(readline(line, LINE_SIZE, fp)) {
            ret = -1;
            goto out;
        }
    } while(strcmp(line, headerend) != 0);

    if(!b64data) {
        return -1;
    }

    if(_libssh2_base64_decode(session, (char **) data, datalen,
                              b64data, b64datalen)) {
        ret = -1;
        goto out;
    }

    if(method) {
#if LIBSSH2_MD5_PEM
        /* Set up decryption */
        int free_iv = 0, free_secret = 0, len_decrypted = 0, padding = 0;
        int blocksize = method->blocksize;
        void *abstract;
        unsigned char secret[2*MD5_DIGEST_LENGTH];
        libssh2_md5_ctx fingerprint_ctx;

        /* Perform key derivation (PBKDF1/MD5) */
        if(!libssh2_md5_init(&fingerprint_ctx) ||
           !libssh2_md5_update(fingerprint_ctx, passphrase,
                               strlen((char *)passphrase)) ||
           !libssh2_md5_update(fingerprint_ctx, iv, 8) ||
           !libssh2_md5_final(fingerprint_ctx, secret)) {
            ret = -1;
            goto out;
        }
        if(method->secret_len > MD5_DIGEST_LENGTH) {
            if(!libssh2_md5_init(&fingerprint_ctx) ||
               !libssh2_md5_update(fingerprint_ctx,
                                   secret, MD5_DIGEST_LENGTH) ||
               !libssh2_md5_update(fingerprint_ctx,
                                   passphrase, strlen((char *)passphrase)) ||
               !libssh2_md5_update(fingerprint_ctx, iv, 8) ||
               !libssh2_md5_final(fingerprint_ctx,
                                  secret + MD5_DIGEST_LENGTH)) {
                ret = -1;
                goto out;
            }
        }

        /* Initialize the decryption */
        if(method->init(session, method, iv, &free_iv, secret,
                         &free_secret, 0, &abstract)) {
            _libssh2_explicit_zero((char *)secret, sizeof(secret));
            LIBSSH2_FREE(session, data);
            ret = -1;
            goto out;
        }

        if(free_secret) {
            _libssh2_explicit_zero((char *)secret, sizeof(secret));
        }

        /* Do the actual decryption */
        if((*datalen % blocksize) != 0) {
            _libssh2_explicit_zero((char *)secret, sizeof(secret));
            method->dtor(session, &abstract);
            _libssh2_explicit_zero(*data, *datalen);
            LIBSSH2_FREE(session, *data);
            ret = -1;
            goto out;
        }

        if(method->flags & LIBSSH2_CRYPT_FLAG_REQUIRES_FULL_PACKET) {
            if(method->crypt(session, 0, *data, *datalen, &abstract, 0)) {
                ret = LIBSSH2_ERROR_DECRYPT;
                _libssh2_explicit_zero((char *)secret, sizeof(secret));
                method->dtor(session, &abstract);
                _libssh2_explicit_zero(*data, *datalen);
                LIBSSH2_FREE(session, *data);
                goto out;
            }
        }
        else {
            while(len_decrypted <= (int)*datalen - blocksize) {
                if(method->crypt(session, 0, *data + len_decrypted, blocksize,
                                &abstract,
                                len_decrypted == 0 ? FIRST_BLOCK :
                                ((len_decrypted == (int)*datalen - blocksize) ?
                                 LAST_BLOCK : MIDDLE_BLOCK)
                                )) {
                    ret = LIBSSH2_ERROR_DECRYPT;
                    _libssh2_explicit_zero((char *)secret, sizeof(secret));
                    method->dtor(session, &abstract);
                    _libssh2_explicit_zero(*data, *datalen);
                    LIBSSH2_FREE(session, *data);
                    goto out;
                }

                len_decrypted += blocksize;
            }
        }

        /* Account for padding */
        padding = (*data)[*datalen - 1];
        memset(&(*data)[*datalen-padding], 0, padding);
        *datalen -= padding;

        /* Clean up */
        _libssh2_explicit_zero((char *)secret, sizeof(secret));
        method->dtor(session, &abstract);
#else
        ret = -1;
        goto out;
#endif
    }

    ret = 0;
out:
    if(b64data) {
        _libssh2_explicit_zero(b64data, b64datalen);
        LIBSSH2_FREE(session, b64data);
    }
    return ret;
}

int
_libssh2_pem_parse_memory(LIBSSH2_SESSION * session,
                          const char *headerbegin,
                          const char *headerend,
                          const char *filedata, size_t filedata_len,
                          unsigned char **data, size_t *datalen)
{
    char line[LINE_SIZE];
    char *b64data = NULL;
    size_t b64datalen = 0;
    size_t off = 0;
    int ret;

    do {
        *line = '\0';

        if(readline_memory(line, LINE_SIZE, filedata, filedata_len, &off)) {
            return -1;
        }
    } while(strcmp(line, headerbegin) != 0);

    *line = '\0';

    do {
        if(*line) {
            char *tmp;
            size_t linelen;

            linelen = strlen(line);
            tmp = LIBSSH2_REALLOC(session, b64data, b64datalen + linelen);
            if(!tmp) {
                _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                               "Unable to allocate memory for PEM parsing");
                ret = -1;
                goto out;
            }
            memcpy(tmp + b64datalen, line, linelen);
            b64data = tmp;
            b64datalen += linelen;
        }

        *line = '\0';

        if(readline_memory(line, LINE_SIZE, filedata, filedata_len, &off)) {
            ret = -1;
            goto out;
        }
    } while(strcmp(line, headerend) != 0);

    if(!b64data) {
        return -1;
    }

    if(_libssh2_base64_decode(session, (char **) data, datalen,
                              b64data, b64datalen)) {
        ret = -1;
        goto out;
    }

    ret = 0;
out:
    if(b64data) {
        _libssh2_explicit_zero(b64data, b64datalen);
        LIBSSH2_FREE(session, b64data);
    }
    return ret;
}

/* OpenSSH formatted keys */
#define AUTH_MAGIC "openssh-key-v1"
#define OPENSSH_HEADER_BEGIN "-----BEGIN OPENSSH PRIVATE KEY-----"
#define OPENSSH_HEADER_END "-----END OPENSSH PRIVATE KEY-----"

static int
_libssh2_openssh_pem_parse_data(LIBSSH2_SESSION * session,
                                const unsigned char *passphrase,
                                const char *b64data, size_t b64datalen,
                                struct string_buf **decrypted_buf)
{
    const LIBSSH2_CRYPT_METHOD *method = NULL;
    struct string_buf decoded, decrypted, kdf_buf;
    unsigned char *ciphername = NULL;
    unsigned char *kdfname = NULL;
    unsigned char *kdf = NULL;
    unsigned char *buf = NULL;
    unsigned char *salt = NULL;
    uint32_t nkeys, check1, check2;
    uint32_t rounds = 0;
    unsigned char *key = NULL;
    unsigned char *key_part = NULL;
    unsigned char *iv_part = NULL;
    unsigned char *f = NULL;
    size_t f_len = 0;
    int ret = 0, keylen = 0, ivlen = 0, total_len = 0;
    size_t kdf_len = 0, tmp_len = 0, salt_len = 0;

    if(decrypted_buf)
        *decrypted_buf = NULL;

    /* decode file */
    if(_libssh2_base64_decode(session, (char **)&f, &f_len,
                              b64data, b64datalen)) {
        ret = -1;
        goto out;
    }

    /* Parse the file */
    decoded.data = (unsigned char *)f;
    decoded.dataptr = (unsigned char *)f;
    decoded.len = f_len;

    if(decoded.len < strlen(AUTH_MAGIC)) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO, "key too short");
        goto out;
    }

    if(strncmp((char *) decoded.dataptr, AUTH_MAGIC,
               strlen(AUTH_MAGIC)) != 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "key auth magic mismatch");
        goto out;
    }

    decoded.dataptr += strlen(AUTH_MAGIC) + 1;

    if(_libssh2_get_string(&decoded, &ciphername, &tmp_len) ||
       tmp_len == 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "ciphername is missing");
        goto out;
    }

    if(_libssh2_get_string(&decoded, &kdfname, &tmp_len) ||
       tmp_len == 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "kdfname is missing");
        goto out;
    }

    if(_libssh2_get_string(&decoded, &kdf, &kdf_len)) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "kdf is missing");
        goto out;
    }
    else {
        kdf_buf.data = kdf;
        kdf_buf.dataptr = kdf;
        kdf_buf.len = kdf_len;
    }

    if((!passphrase || strlen((const char *)passphrase) == 0) &&
        strcmp((const char *)ciphername, "none") != 0) {
        /* passphrase required */
        ret = LIBSSH2_ERROR_KEYFILE_AUTH_FAILED;
        goto out;
    }

    if(strcmp((const char *)kdfname, "none") != 0 &&
       strcmp((const char *)kdfname, "bcrypt") != 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "unknown cipher");
        goto out;
    }

    if(!strcmp((const char *)kdfname, "none") &&
       strcmp((const char *)ciphername, "none") != 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "invalid format");
        goto out;
    }

    if(_libssh2_get_u32(&decoded, &nkeys) != 0 || nkeys != 1) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "Multiple keys are unsupported");
        goto out;
    }

    /* unencrypted public key */

    if(_libssh2_get_string(&decoded, &buf, &tmp_len) || tmp_len == 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "Invalid private key; "
                             "expect embedded public key");
        goto out;
    }

    if(_libssh2_get_string(&decoded, &buf, &tmp_len) || tmp_len == 0) {
        ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                             "Private key data not found");
        goto out;
    }

    /* decode encrypted private key */
    decrypted.data = decrypted.dataptr = buf;
    decrypted.len = tmp_len;

    if(ciphername && strcmp((const char *)ciphername, "none") != 0) {
        const LIBSSH2_CRYPT_METHOD **all_methods, *cur_method;

        all_methods = libssh2_crypt_methods();
        /* !checksrc! disable EQUALSNULL 1 */
        while((cur_method = *all_methods++) != NULL) {
            if(*cur_method->name &&
                memcmp(ciphername, cur_method->name,
                       strlen(cur_method->name)) == 0) {
                    method = cur_method;
                }
        }

        /* None of the available crypt methods were able to decrypt the key */

        if(!method) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "No supported cipher found");
            goto out;
        }
    }

    if(method) {
        int free_iv = 0, free_secret = 0, len_decrypted = 0;
        int blocksize;
        void *abstract = NULL;

        keylen = method->secret_len;
        ivlen = method->iv_len;
        total_len = keylen + ivlen;

        key = LIBSSH2_CALLOC(session, total_len);
        if(!key) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Could not alloc key");
            goto out;
        }

        if(strcmp((const char *)kdfname, "bcrypt") == 0 && passphrase) {
            if((_libssh2_get_string(&kdf_buf, &salt, &salt_len)) ||
                (_libssh2_get_u32(&kdf_buf, &rounds) != 0)) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                     "kdf contains unexpected values");
                LIBSSH2_FREE(session, key);
                goto out;
            }

            if(_libssh2_bcrypt_pbkdf((const char *)passphrase,
                                     strlen((const char *)passphrase),
                                     salt, salt_len, key,
                                     keylen + ivlen, rounds) < 0) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_DECRYPT,
                                     "invalid format");
                LIBSSH2_FREE(session, key);
                goto out;
            }
        }
        else {
            ret = _libssh2_error(session, LIBSSH2_ERROR_KEYFILE_AUTH_FAILED,
                                 "bcrypted without passphrase");
            LIBSSH2_FREE(session, key);
            goto out;
        }

        /* Set up decryption */
        blocksize = method->blocksize;

        key_part = LIBSSH2_CALLOC(session, keylen);
        if(!key_part) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Could not alloc key part");
            goto out;
        }

        iv_part = LIBSSH2_CALLOC(session, ivlen);
        if(!iv_part) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Could not alloc iv part");
            goto out;
        }

        memcpy(key_part, key, keylen);
        memcpy(iv_part, key + keylen, ivlen);

        /* Initialize the decryption */
        if(method->init(session, method, iv_part, &free_iv, key_part,
                        &free_secret, 0, &abstract)) {
            ret = LIBSSH2_ERROR_DECRYPT;
            goto out;
        }

        /* Do the actual decryption */
        if((decrypted.len % blocksize) != 0) {
            method->dtor(session, &abstract);
            ret = LIBSSH2_ERROR_DECRYPT;
            goto out;
        }

        if(method->flags & LIBSSH2_CRYPT_FLAG_REQUIRES_FULL_PACKET) {
            if(method->crypt(session, 0, decrypted.data,
                             decrypted.len,
                             &abstract,
                             MIDDLE_BLOCK)) {
                ret = LIBSSH2_ERROR_DECRYPT;
                method->dtor(session, &abstract);
                goto out;
            }
        }
        else {
            while((size_t)len_decrypted <= decrypted.len - blocksize) {
                /* We always pass MIDDLE_BLOCK here because OpenSSH Key Files
                 * do not use AAD to authenticate the length.
                 * Furthermore, the authentication tag is appended after the
                 * encrypted key, and the length of the authentication tag is
                 * not included in the key length, so we check it after the
                 * loop.
                 */
                if(method->crypt(session, 0, decrypted.data + len_decrypted,
                                 blocksize,
                                 &abstract,
                                 MIDDLE_BLOCK)) {
                    ret = LIBSSH2_ERROR_DECRYPT;
                    method->dtor(session, &abstract);
                    goto out;
                }

                len_decrypted += blocksize;
            }

            /* No padding */

            /* for the AES GCM methods, the 16 byte authentication tag is
             * appended to the encrypted key */
            if(strcmp(method->name, "aes256-gcm@openssh.com") == 0 ||
               strcmp(method->name, "aes128-gcm@openssh.com") == 0) {
                if(!_libssh2_check_length(&decoded, 16)) {
                    ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                         "GCM auth tag missing");
                    method->dtor(session, &abstract);
                    goto out;
                }
                if(method->crypt(session, 0, decoded.dataptr, 16, &abstract,
                                 LAST_BLOCK)) {
                    ret = _libssh2_error(session, LIBSSH2_ERROR_DECRYPT,
                                         "GCM auth tag invalid");
                    method->dtor(session, &abstract);
                    goto out;
                }
                decoded.dataptr += 16;
            }
        }

        method->dtor(session, &abstract);
    }

    /* Check random bytes match */

    if(_libssh2_get_u32(&decrypted, &check1) != 0 ||
       _libssh2_get_u32(&decrypted, &check2) != 0 ||
       check1 != check2) {
        _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                       "Private key unpack failed (correct password?)");
        ret = LIBSSH2_ERROR_KEYFILE_AUTH_FAILED;
        goto out;
    }

    if(decrypted_buf) {
        /* copy data to out-going buffer */
        struct string_buf *out_buf = _libssh2_string_buf_new(session);
        if(!out_buf) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate memory for "
                                 "decrypted struct");
            goto out;
        }

        out_buf->data = LIBSSH2_CALLOC(session, decrypted.len);
        if(!out_buf->data) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                 "Unable to allocate memory for "
                                 "decrypted struct");
            _libssh2_string_buf_free(session, out_buf);
            goto out;
        }
        memcpy(out_buf->data, decrypted.data, decrypted.len);
        out_buf->dataptr = out_buf->data +
            (decrypted.dataptr - decrypted.data);
        out_buf->len = decrypted.len;

        *decrypted_buf = out_buf;
    }

out:

    /* Clean up */
    if(key) {
        _libssh2_explicit_zero(key, total_len);
        LIBSSH2_FREE(session, key);
    }
    if(key_part) {
        _libssh2_explicit_zero(key_part, keylen);
        LIBSSH2_FREE(session, key_part);
    }
    if(iv_part) {
        _libssh2_explicit_zero(iv_part, ivlen);
        LIBSSH2_FREE(session, iv_part);
    }
    if(f) {
        _libssh2_explicit_zero(f, f_len);
        LIBSSH2_FREE(session, f);
    }

    return ret;
}

int
_libssh2_openssh_pem_parse(LIBSSH2_SESSION * session,
                           const unsigned char *passphrase,
                           FILE * fp, struct string_buf **decrypted_buf)
{
    char line[LINE_SIZE];
    char *b64data = NULL;
    size_t b64datalen = 0;
    int ret = 0;

    /* read file */

    do {
        *line = '\0';

        if(readline(line, LINE_SIZE, fp)) {
            return -1;
        }
    } while(strcmp(line, OPENSSH_HEADER_BEGIN) != 0);

    if(readline(line, LINE_SIZE, fp)) {
        return -1;
    }

    do {
        if(*line) {
            char *tmp;
            size_t linelen;

            linelen = strlen(line);
            tmp = LIBSSH2_REALLOC(session, b64data, b64datalen + linelen);
            if(!tmp) {
                _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                               "Unable to allocate memory for PEM parsing");
                ret = -1;
                goto out;
            }
            memcpy(tmp + b64datalen, line, linelen);
            b64data = tmp;
            b64datalen += linelen;
        }

        *line = '\0';

        if(readline(line, LINE_SIZE, fp)) {
            ret = -1;
            goto out;
        }
    } while(strcmp(line, OPENSSH_HEADER_END) != 0);

    if(!b64data) {
        return -1;
    }

    ret = _libssh2_openssh_pem_parse_data(session,
                                          passphrase,
                                          (const char *)b64data,
                                          b64datalen,
                                          decrypted_buf);

    if(b64data) {
        _libssh2_explicit_zero(b64data, b64datalen);
        LIBSSH2_FREE(session, b64data);
    }

out:

    return ret;
}

int
_libssh2_openssh_pem_parse_memory(LIBSSH2_SESSION * session,
                                  const unsigned char *passphrase,
                                  const char *filedata, size_t filedata_len,
                                  struct string_buf **decrypted_buf)
{
    char line[LINE_SIZE];
    char *b64data = NULL;
    size_t b64datalen = 0;
    size_t off = 0;
    int ret;

    if(!filedata || filedata_len <= 0)
        return _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                              "Error parsing PEM: filedata missing");

    do {

        *line = '\0';

        if(off >= filedata_len)
            return _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                  "Error parsing PEM: "
                                  "OpenSSH header not found");

        if(readline_memory(line, LINE_SIZE, filedata, filedata_len, &off)) {
            return -1;
        }
    } while(strcmp(line, OPENSSH_HEADER_BEGIN) != 0);

    *line = '\0';

    do {
        if(*line) {
            char *tmp;
            size_t linelen;

            linelen = strlen(line);
            tmp = LIBSSH2_REALLOC(session, b64data, b64datalen + linelen);
            if(!tmp) {
                ret = _libssh2_error(session, LIBSSH2_ERROR_ALLOC,
                                     "Unable to allocate memory for "
                                     "PEM parsing");
                goto out;
            }
            memcpy(tmp + b64datalen, line, linelen);
            b64data = tmp;
            b64datalen += linelen;
        }

        *line = '\0';

        if(off >= filedata_len) {
            ret = _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                                 "Error parsing PEM: offset out of bounds");
            goto out;
        }

        if(readline_memory(line, LINE_SIZE, filedata, filedata_len, &off)) {
            ret = -1;
            goto out;
        }
    } while(strcmp(line, OPENSSH_HEADER_END) != 0);

    if(!b64data)
        return _libssh2_error(session, LIBSSH2_ERROR_PROTO,
                              "Error parsing PEM: base 64 data missing");

    ret = _libssh2_openssh_pem_parse_data(session, passphrase, b64data,
                                          b64datalen, decrypted_buf);

out:
    if(b64data) {
        _libssh2_explicit_zero(b64data, b64datalen);
        LIBSSH2_FREE(session, b64data);
    }
    return ret;

}

static int
read_asn1_length(const unsigned char *data,
                 size_t datalen, size_t *len)
{
    unsigned int lenlen;
    int nextpos;

    if(datalen < 1) {
        return -1;
    }
    *len = data[0];

    if(*len >= 0x80) {
        lenlen = *len & 0x7F;
        *len = data[1];
        if(1 + lenlen > datalen) {
            return -1;
        }
        if(lenlen > 1) {
            *len <<= 8;
            *len |= data[2];
        }
    }
    else {
        lenlen = 0;
    }

    nextpos = 1 + lenlen;
    if(lenlen > 2 || 1 + lenlen + *len > datalen) {
        return -1;
    }

    return nextpos;
}

int
_libssh2_pem_decode_sequence(unsigned char **data, size_t *datalen)
{
    size_t len;
    int lenlen;

    if(*datalen < 1) {
        return -1;
    }

    if((*data)[0] != '\x30') {
        return -1;
    }

    (*data)++;
    (*datalen)--;

    lenlen = read_asn1_length(*data, *datalen, &len);
    if(lenlen < 0 || lenlen + len != *datalen) {
        return -1;
    }

    *data += lenlen;
    *datalen -= lenlen;

    return 0;
}

int
_libssh2_pem_decode_integer(unsigned char **data, size_t *datalen,
                            unsigned char **i, unsigned int *ilen)
{
    size_t len;
    int lenlen;

    if(*datalen < 1) {
        return -1;
    }

    if((*data)[0] != '\x02') {
        return -1;
    }

    (*data)++;
    (*datalen)--;

    lenlen = read_asn1_length(*data, *datalen, &len);
    if(lenlen < 0 || lenlen + len > *datalen) {
        return -1;
    }

    *data += lenlen;
    *datalen -= lenlen;

    *i = *data;
    *ilen = (unsigned int)len;

    *data += len;
    *datalen -= len;

    return 0;
}
