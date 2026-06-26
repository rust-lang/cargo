/* Copyright (C) Alexander Lamaison
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

#include "session_fixture.h"
#include "openssh_fixture.h"

#ifdef HAVE_SYS_SOCKET_H
#include <sys/socket.h>
#endif
#ifdef HAVE_UNISTD_H
#include <unistd.h>
#endif
#ifdef HAVE_ARPA_INET_H
#include <arpa/inet.h>
#endif
#ifdef HAVE_NETINET_IN_H
#include <netinet/in.h>
#endif

#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <ctype.h>

#if defined(_WIN32) && defined(_WIN64)
#define LIBSSH2_SOCKET_MASK "%lld"
#else
#define LIBSSH2_SOCKET_MASK "%d"
#endif

#ifdef LIBSSH2_WINDOWS_UWP
#define popen(x, y) (NULL)
#define pclose(x) (-1)
#elif defined(_WIN32)
#define popen _popen
#define pclose _pclose
#endif

static int have_docker = 0;

int openssh_fixture_have_docker(void)
{
    return have_docker;
}

static int run_command_varg(char **output, const char *command, va_list args)
    LIBSSH2_PRINTF(2, 0);

static int run_command_varg(char **output, const char *command, va_list args)
{
    static const char redirect_stderr[] = "%s 2>&1";

    FILE *pipe;
    char command_buf[BUFSIZ];
    char buf[BUFSIZ + sizeof(redirect_stderr)];
    int ret;
    size_t buf_len;

    if(output) {
        *output = NULL;
    }

    /* Format the command string */
    ret = vsnprintf(command_buf, sizeof(command_buf), command, args);
    if(ret < 0 || ret >= BUFSIZ) {
        fprintf(stderr, "Unable to format command (%s)\n", command);
        return -1;
    }

    /* Rewrite the command to redirect stderr to stdout to we can output it */
    if(strlen(command_buf) + strlen(redirect_stderr) >= sizeof(buf)) {
        fprintf(stderr, "Unable to rewrite command (%s)\n", command);
        return -1;
    }

    ret = snprintf(buf, sizeof(buf), redirect_stderr, command_buf);
    if(ret < 0 || ret >= BUFSIZ) {
        fprintf(stderr, "Unable to rewrite command (%s)\n", command);
        return -1;
    }

    fprintf(stdout, "Command: %s\n", command_buf);
    pipe = popen(buf, "r");
    if(!pipe) {
        fprintf(stderr, "Unable to execute command '%s'\n", command);
        return -1;
    }
    buf[0] = 0;
    buf_len = 0;
    while(buf_len < (sizeof(buf) - 1) &&
        fgets(&buf[buf_len], (int)(sizeof(buf) - buf_len), pipe)) {
        buf_len = strlen(buf);
    }

    ret = pclose(pipe);
    if(ret) {
        fprintf(stderr, "Error running command '%s' (exit %d): %s\n",
                command, ret, buf);
    }

    if(output) {
        /* command output may contain a trailing newline, so we trim
         * whitespace here */
        size_t end = strlen(buf);
        while(end > 0 && isspace((int)buf[end - 1])) {
            buf[end - 1] = '\0';
        }

        *output = strdup(buf);
    }
    return ret;
}

static int run_command(char **output, const char *command, ...)
    LIBSSH2_PRINTF(2, 3);

static int run_command(char **output, const char *command, ...)
{
    va_list args;
    int ret;

    va_start(args, command);
    ret = run_command_varg(output, command, args);
    va_end(args);

    return ret;
}

static const char *openssh_server_image(void)
{
    return getenv("OPENSSH_SERVER_IMAGE");
}

static int build_openssh_server_docker_image(void)
{
    if(have_docker) {
        const char *container_image_name = openssh_server_image();
        if(container_image_name) {
            int ret = run_command(NULL, "docker pull %s",
                                  container_image_name);
            if(ret == 0) {
                ret = run_command(NULL, "docker tag %s libssh2/openssh_server",
                                  container_image_name);
                if(ret == 0) {
                    return ret;
                }
            }
        }
        return run_command(NULL,
                           "docker build --quiet -t libssh2/openssh_server %s",
                           srcdir_path("openssh_server"));
    }
    else {
        return 0;
    }
}

static const char *openssh_server_port(void)
{
    return getenv("OPENSSH_SERVER_PORT");
}

static int start_openssh_server(char **container_id_out)
{
    if(have_docker) {
        const char *container_host_port = openssh_server_port();
        if(container_host_port) {
            return run_command(container_id_out,
                               "docker run --rm -d -p %s:22 "
                               "libssh2/openssh_server",
                               container_host_port);
        }

        return run_command(container_id_out,
                           "docker run --rm -d -p 22 "
                           "libssh2/openssh_server");
    }
    else {
        *container_id_out = strdup("");
        return 0;
    }
}

static int stop_openssh_server(char *container_id)
{
    if(have_docker) {
        return run_command(NULL, "docker stop %s", container_id);
    }
    else {
        return 0;
    }
}

static const char *docker_machine_name(void)
{
    return getenv("DOCKER_MACHINE_NAME");
}

static int is_running_inside_a_container(void)
{
#ifdef _WIN32
    return 0;
#else
    const char *cgroup_filename = "/proc/self/cgroup";
    FILE *f;
    char *line = NULL;
    size_t len = 0;
    ssize_t read;
    int found = 0;
    f = fopen(cgroup_filename, "r");
    if(!f) {
        /* Don't go further, we are not in a container */
        return 0;
    }
    while((read = getline(&line, &len, f)) != -1) {
        if(strstr(line, "docker")) {
            found = 1;
            break;
        }
    }
    fclose(f);
    free(line);
    return found;
#endif
}

static void portable_sleep(unsigned int seconds)
{
#ifdef _WIN32
    Sleep(seconds);
#else
    sleep(seconds);
#endif
}

static int ip_address_from_container(char *container_id, char **ip_address_out)
{
    const char *active_docker_machine = docker_machine_name();
    if(active_docker_machine) {

        /* This can be flaky when tests run in parallel (see
           https://github.com/docker/machine/issues/2612), so we retry a few
           times with exponential backoff if it fails */
        int attempt_no = 0;
        unsigned int wait_time = 500;
        for(;;) {
            int ret = run_command(ip_address_out, "docker-machine ip %s",
                                  active_docker_machine);
            if(ret == 0) {
                return 0;
            }
            else if(attempt_no > 5) {
                fprintf(
                    stderr,
                    "Unable to get IP from docker-machine after %d attempts\n",
                    attempt_no);
                return -1;
            }
            else {
                portable_sleep(wait_time);
                ++attempt_no;
                wait_time *= 2;
            }
        }
    }
    else {
        if(is_running_inside_a_container()) {
            return run_command(ip_address_out,
                               "docker inspect --format "
                               "\"{{ .NetworkSettings.IPAddress }}\""
                               " %s",
                               container_id);
        }
        else {
            return run_command(ip_address_out,
                               "docker inspect --format "
                               "\"{{ index (index (index "
                               ".NetworkSettings.Ports "
                               "\\\"22/tcp\\\") 0) \\\"HostIp\\\" }}\" %s",
                               container_id);
        }
    }
}

static int port_from_container(char *container_id, char **port_out)
{
    if(is_running_inside_a_container()) {
        *port_out = strdup("22");
        return 0;
    }
    else {
        return run_command(port_out,
                           "docker inspect --format "
                           "\"{{ index (index (index .NetworkSettings.Ports "
                           "\\\"22/tcp\\\") 0) \\\"HostPort\\\" }}\" %s",
                           container_id);
    }
}

static libssh2_socket_t open_socket_to_container(char *container_id)
{
    char *ip_address = NULL;
    char *port_string = NULL;
    uint32_t hostaddr;
    libssh2_socket_t sock;
    struct sockaddr_in sin;
    unsigned int counter;
    libssh2_socket_t ret = LIBSSH2_INVALID_SOCKET;

    if(have_docker) {
        int res;
        res = ip_address_from_container(container_id, &ip_address);
        if(res) {
            fprintf(stderr, "Failed to get IP address for container %s\n",
                    container_id);
            goto cleanup;
        }

        res = port_from_container(container_id, &port_string);
        if(res) {
            fprintf(stderr, "Failed to get port for container %s\n",
                    container_id);
            goto cleanup;
        }
    }
    else {
        const char *env;
        env = getenv("OPENSSH_SERVER_HOST");
        if(!env) {
            env = "127.0.0.1";
        }
        ip_address = strdup(env);
        env = openssh_server_port();
        if(!env) {
            env = "4711";
        }
        port_string = strdup(env);
    }

    /* 0.0.0.0 is returned by Docker for Windows, because the container
       is reachable from anywhere. But we cannot connect to 0.0.0.0,
       instead we assume localhost and try to connect to 127.0.0.1. */
    if(ip_address && strcmp(ip_address, "0.0.0.0") == 0) {
        free(ip_address);
        ip_address = strdup("127.0.0.1");
    }

    hostaddr = inet_addr(ip_address);
    if(hostaddr == (uint32_t)(-1)) {
        fprintf(stderr, "Failed to convert %s host address\n", ip_address);
        goto cleanup;
    }

    sock = socket(AF_INET, SOCK_STREAM, 0);
    if(sock == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr,
                "Failed to open socket (" LIBSSH2_SOCKET_MASK ")\n", sock);
        goto cleanup;
    }

    sin.sin_family = AF_INET;
    sin.sin_port = htons((unsigned short)strtol(port_string, NULL, 0));
    sin.sin_addr.s_addr = hostaddr;

    for(counter = 0; counter < 3; ++counter) {
        if(connect(sock, (struct sockaddr*)(&sin),
                   sizeof(struct sockaddr_in))) {
            fprintf(stderr,
                    "Connection to %s:%s attempt #%d failed: retrying...\n",
                    ip_address, port_string, counter);
            portable_sleep(1 + 2*counter);
        }
        else {
            ret = sock;
            break;
        }
    }
    if(ret == LIBSSH2_INVALID_SOCKET) {
        fprintf(stderr, "Failed to connect to %s:%s\n",
                ip_address, port_string);
        goto cleanup;
    }

cleanup:
    free(ip_address);
    free(port_string);

    return ret;
}

static void close_socket_to_container(libssh2_socket_t sock)
{
    if(sock != LIBSSH2_INVALID_SOCKET) {
        shutdown(sock, 2 /* SHUT_RDWR */);
#ifdef _WIN32
        closesocket(sock);
#else
        close(sock);
#endif
    }
}

static char *running_container_id = NULL;

int start_openssh_fixture(void)
{
    int ret;
#ifdef _WIN32
    WSADATA wsadata;

    ret = WSAStartup(MAKEWORD(2, 0), &wsadata);
    if(ret) {
        fprintf(stderr, "WSAStartup failed with error: %d\n", ret);
        return 1;
    }
#endif

    have_docker = !getenv("OPENSSH_NO_DOCKER");

    ret = build_openssh_server_docker_image();
    if(!ret) {
        return start_openssh_server(&running_container_id);
    }
    else {
        fprintf(stderr, "Failed to build docker image\n");
        return ret;
    }
}

void stop_openssh_fixture(void)
{
    if(running_container_id) {
        stop_openssh_server(running_container_id);
        free(running_container_id);
        running_container_id = NULL;
    }
    else if(have_docker) {
        fprintf(stderr, "Cannot stop container - none started\n");
    }

#ifdef _WIN32
    WSACleanup();
#endif
}

libssh2_socket_t open_socket_to_openssh_server(void)
{
    return open_socket_to_container(running_container_id);
}

void close_socket_to_openssh_server(libssh2_socket_t sock)
{
    close_socket_to_container(sock);
}
