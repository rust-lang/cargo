#ifndef LIBSSH2_MISC_H
#define LIBSSH2_MISC_H
/* Copyright (C) Daniel Stenberg
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

#ifdef LIBSSH2_NO_CLEAR_MEMORY
#define _libssh2_explicit_zero(buf, size) do { \
                                              (void)(buf); \
                                              (void)(size); \
                                          } while(0)
#elif defined(_WIN32)
#define _libssh2_explicit_zero(buf, size) SecureZeroMemory(buf, size)
#elif defined(HAVE_EXPLICIT_BZERO)
#define _libssh2_explicit_zero(buf, size) explicit_bzero(buf, size)
#elif defined(HAVE_EXPLICIT_MEMSET)
#define _libssh2_explicit_zero(buf, size) (void)explicit_memset(buf, 0, size)
#elif defined(HAVE_MEMSET_S)
#define _libssh2_explicit_zero(buf, size) (void)memset_s(buf, size, 0, size)
#else
#define LIBSSH2_MEMZERO
void _libssh2_memzero(void *buf, size_t size);
#define _libssh2_explicit_zero(buf, size) _libssh2_memzero(buf, size)
#endif

struct list_head {
    struct list_node *last;
    struct list_node *first;
};

struct list_node {
    struct list_node *next;
    struct list_node *prev;
    struct list_head *head;
};

struct string_buf {
    unsigned char *data;
    unsigned char *dataptr;
    size_t len;
};

int _libssh2_error_flags(LIBSSH2_SESSION* session, int errcode,
                         const char *errmsg, int errflags);
int _libssh2_error(LIBSSH2_SESSION* session, int errcode, const char *errmsg);

#ifdef _WIN32
/* Convert Win32 WSAGetLastError to errno equivalent */
int _libssh2_wsa2errno(void);
#endif

void _libssh2_list_init(struct list_head *head);

/* add a node last in the list */
void _libssh2_list_add(struct list_head *head,
                       struct list_node *entry);

/* return the "first" node in the list this head points to */
void *_libssh2_list_first(struct list_head *head);

/* return the next node in the list */
void *_libssh2_list_next(struct list_node *node);

/* return the prev node in the list */
void *_libssh2_list_prev(struct list_node *node);

/* remove this node from the list */
void _libssh2_list_remove(struct list_node *entry);

int _libssh2_base64_decode(LIBSSH2_SESSION *session,
                           char **data, size_t *datalen,
                           const char *src, size_t src_len);
size_t _libssh2_base64_encode(LIBSSH2_SESSION *session,
                              const char *inp, size_t insize, char **outptr);

uint32_t _libssh2_ntohu32(const unsigned char *buf);
libssh2_uint64_t _libssh2_ntohu64(const unsigned char *buf);
void _libssh2_htonu32(unsigned char *buf, uint32_t val);
void _libssh2_store_u32(unsigned char **buf, uint32_t value);
void _libssh2_store_u64(unsigned char **buf, libssh2_uint64_t value);
int _libssh2_store_str(unsigned char **buf, const char *str, size_t len);
int _libssh2_store_bignum2_bytes(unsigned char **buf,
                                 const unsigned char *bytes,
                                 size_t len);
void *_libssh2_calloc(LIBSSH2_SESSION *session, size_t size);

struct string_buf *_libssh2_string_buf_new(LIBSSH2_SESSION *session);
void _libssh2_string_buf_free(LIBSSH2_SESSION *session,
                              struct string_buf *buf);
int _libssh2_get_boolean(struct string_buf *buf, unsigned char *out);
int _libssh2_get_byte(struct string_buf *buf, unsigned char *out);
int _libssh2_get_u32(struct string_buf *buf, uint32_t *out);
int _libssh2_get_u64(struct string_buf *buf, libssh2_uint64_t *out);
int _libssh2_match_string(struct string_buf *buf, const char *match);
int _libssh2_get_string(struct string_buf *buf, unsigned char **outbuf,
                        size_t *outlen);
int _libssh2_copy_string(LIBSSH2_SESSION* session, struct string_buf *buf,
                         unsigned char **outbuf, size_t *outlen);
int _libssh2_get_bignum_bytes(struct string_buf *buf, unsigned char **outbuf,
                              size_t *outlen);
int _libssh2_check_length(struct string_buf *buf, size_t requested_len);
int _libssh2_eob(struct string_buf *buf);

void _libssh2_xor_data(unsigned char *output,
                       const unsigned char *input1,
                       const unsigned char *input2,
                       size_t length);

void _libssh2_aes_ctr_increment(unsigned char *ctr, size_t length);

#endif /* LIBSSH2_MISC_H */
