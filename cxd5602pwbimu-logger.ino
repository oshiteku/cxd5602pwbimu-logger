/****************************************************************************
 * cxd5602pwbimu-logger.ino
 *
 *   Copyright 2025 Sony Semiconductor Solutions Corporation
 *   Copyright 2025 oshiteku
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in
 *    the documentation and/or other materials provided with the
 *    distribution.
 * 3. Neither the name of Sony Semiconductor Solutions Corporation nor
 *    the names of its contributors may be used to endorse or promote
 *    products derived from this software without specific prior written
 *    permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
 * "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
 * LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS
 * FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE
 * COPYRIGHT OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT,
 * INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS
 * OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
 * AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN
 * ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 *
 ****************************************************************************/

/****************************************************************************
 * Included Files
 ****************************************************************************/

#include <nuttx/config.h>
#include <stdio.h>
#include <poll.h>
#include <inttypes.h>
#include <sys/ioctl.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <nuttx/sensors/cxd5602pwbimu.h>
#include <arch/board/cxd56_cxd5602pwbimu.h>

/****************************************************************************
 * Pre-processor Definitions
 ****************************************************************************/

#define SPACE__ "                          "
#define CXD5602PWBIMU_DEVPATH      "/dev/imu0"
#define MAX_NFIFO (4)

/****************************************************************************
 * Private Data Types
 ****************************************************************************/

union conv_f2u_u
{
  float f;
  unsigned int u;
};
typedef union conv_f2u_u conv_f2u_t;

typedef int (*logfunc_t)(cxd5602pwbimu_data_t *, int, void *);

/****************************************************************************
 * Private Data
 ****************************************************************************/

static cxd5602pwbimu_data_t g_data[MAX_NFIFO];

/****************************************************************************
 * Private Functions
 ****************************************************************************/

static int start_sensing(int fd, int rate, int adrange, int gdrange,
                         int nfifos)
{
  cxd5602pwbimu_range_t range;
  int ret;

  /*
   * Set sampling rate. Available values (Hz) are below.
   *
   * 15 (default), 30, 60, 120, 240, 480, 960, 1920
   */

  ret = ioctl(fd, SNIOC_SSAMPRATE, rate);
  if (ret)
    {
      printf("ERROR: Set sampling rate failed. %d\n", errno);
      return 1;
    }

  /*
   * Set dynamic ranges for accelerometer and gyroscope.
   * Available values are below.
   *
   * accel: 2 (default), 4, 8, 16
   * gyro: 125 (default), 250, 500, 1000, 2000, 4000
   */

  range.accel = adrange;
  range.gyro = gdrange;
  ret = ioctl(fd, SNIOC_SDRANGE, (unsigned long)(uintptr_t)&range);
  if (ret)
    {
      printf("ERROR: Set dynamic range failed. %d\n", errno);
      return 1;
    }

  /*
   * Set hardware FIFO threshold.
   * Increasing this value will reduce the frequency with which data is
   * received.
   */

  ret = ioctl(fd, SNIOC_SFIFOTHRESH, nfifos);
  if (ret)
    {
      printf("ERROR: Set sampling rate failed. %d\n", errno);
      return 1;
    }

  /*
   * Start sensing, user can not change the all of configurations.
   */

  ret = ioctl(fd, SNIOC_ENABLE, 1);
  if (ret)
    {
      printf("ERROR: Enable failed. %d\n", errno);
      return 1;
    }

  return 0;
}

static int drop_50msdata(int fd, int samprate, int nfifo)
{
  int cnt = samprate / 20; /* data size of 50ms */

  cnt = ((cnt + nfifo - 1) / nfifo) * nfifo;
  if (cnt == 0) cnt = nfifo;

  while (cnt)
    {
      read(fd, g_data, sizeof(g_data[0]) * nfifo);
      cnt -= nfifo;
    }

  return 0;
}

static int log2filebin(cxd5602pwbimu_data_t *dat, int num, void *arg)
{
  FILE *fp = (FILE *)arg;
  fwrite(dat, sizeof(dat[0]), num, fp);
  return 0;
}

static int log2uart(cxd5602pwbimu_data_t *dat, int num, void *arg)
{
  int i;
  for (i = 0; i < num; i++)
    {
      printf("%08x,%08x,%08x,%08x,"
             "%08x,%08x,%08x,%08x\n",
             dat[i].timestamp,
             *reinterpret_cast<uint32_t*>(&dat[i].temp),
             *reinterpret_cast<uint32_t*>(&dat[i].gx),
             *reinterpret_cast<uint32_t*>(&dat[i].gy),
             *reinterpret_cast<uint32_t*>(&dat[i].gz),
             *reinterpret_cast<uint32_t*>(&dat[i].ax),
             *reinterpret_cast<uint32_t*>(&dat[i].ay),
             *reinterpret_cast<uint32_t*>(&dat[i].az));
    }

  return 0;
}

static int dump_data(int fd, int nfifo,
                     int disp, logfunc_t func, void *arg)
{
  int c;
  int ret;
  struct pollfd fds[2];

  fds[0].fd     = fileno(stdin);
  fds[0].events = POLLIN;
  fds[1].fd     = fd;
  fds[1].events = POLLIN;

  while (1)
    {
      ret = poll(fds, 2, 1000);
      if (ret <= 0)
        {
          printf("Poll error\n");
          return -1;
        }

      if (fds[1].revents & POLLIN)
        {
          ret = read(fd, g_data, sizeof(g_data[0]) * nfifo);
          if (ret == sizeof(g_data[0]) * nfifo)
            {
              func(g_data, nfifo, arg);
            }
          else
            {
              printf("Read error : %d\n", ret);
              return -1;
            }

          if (disp && func != log2uart)
            {
              log2uart(g_data, nfifo, NULL);
            }
        }

      if (fds[0].revents & POLLIN)
        {
          read(fds[0].fd, &c, 1);
          if (c == 'q') return 0;
        }
    }
}

static void print_help(void)
{
  printf("Usage: nsh> pwbimu_logger ([-s] <rate>) ([-a] <acc range>) "
         "([-g] <gyro range>) ([-f] <fifo num>) ([-o] <out dev>) "
         "([-d]) ([-h])\n");
  printf(SPACE__ "-s: Sampling Rate 1920, 960, 480, 240, 120, 60, 30, 15\n"
         SPACE__ "-a: Accel Range 16, 8, 4, 2\n"
         SPACE__ "-g: Gyro Range 4000, 2000, 1000, 500, 250, 125\n"
         SPACE__ "-f: Fifo size 4, 3, 2, 1\n"
         SPACE__ "-o: Output Device 'uart', 'net', "
                 "/path/to/file.bin, /path/to/file.txt\n"
         SPACE__ "-d: Force print the data to UART\n"
         SPACE__ "-h: Show this help\n");
}

/****************************************************************************
 * Main Functions
 ****************************************************************************/

void setup()
{
  int ret = 0;
  int opt;
  int devfd;
  int samprate = 960;
  int arange   = 16;
  int grange   = 500;
  int fifo     = 4;
  int disp     = 0;
  const char *outdev = "uart";
  FILE *fp = NULL;
  logfunc_t logfunc;
  void *logopt = NULL;

  Serial.begin(921600, SERIAL_8E1);
  while (!Serial) {}

  // Use UART for logging
  logfunc = log2uart;

  // Initialize IMU
  // Reference: https://qiita.com/SaChiKaKK/items/50c550782b13fe43061e
  ret = board_cxd5602pwbimu_initialize(5);
  if (ret < 0)
    {
      printf("ERROR: Failed to initialize CXD5602PWBIMU.\n");
      ret = -1;
      goto end_app;
    }

  devfd = open(CXD5602PWBIMU_DEVPATH, O_RDONLY);
  if (devfd < 0)
    {
      printf("Could not open the device:%d\n", devfd);
      ret = -1;
      goto end_app;
    }

  /* Setup Sensor and start drain IMU data */

  printf("Start IMU Data logging: "
         "Rate:%d Accel DRagne:%d Gyro DRange:%d Fifo:%d Outdev:%s\n",
         samprate, arange, grange, fifo, outdev);
  ret = start_sensing(devfd, samprate, arange, grange, fifo);
  if (ret < 0)
    {
      printf("Sensor start error\n");
      ret = -1;
      goto end_app;
    }

  /* Drop first 50ms data, because it is invalid */

  drop_50msdata(devfd, samprate, fifo);

  /* Start capturing IMU data */

  dump_data(devfd, fifo, disp, logfunc, logopt);

end_app:
  if (fp) fclose(fp);
}

void loop() {}
