/*
 * The reader of MapServer's reprojection for G-11's reference
 * (ADR-0038): it reads the list of points named as its one
 * argument, crates/proj/tests/reference/points.tsv, and reprojects each point from its line's
 * source projection into its target projection as MapServer reprojects a shape's point: each
 * projection loaded by msLoadProjectionString of src/mapfile.c, the point reprojected by
 * msProjectPoint of src/mapproject.c, which asks PROJ for the operation through
 * createNormalizedPJ.
 *
 * Its output, on standard output, is the record crates/proj/tests/reference/mapserver.tsv: two
 * header lines, "# PROJ <release>" from proj_info and "# MapServer <commit>" as its build names it,
 * then for each point of the list its line, a tab, and the result as "x<TAB>y" written with %.17g,
 * or "fail" where msProjectPoint fails. Comment lines and empty lines of the list are skipped. A
 * line of the list that does not hold four tab-separated fields, or a projection that does not
 * load, ends the reader with an error.
 *
 * Copyright (c) 1996-2005 Regents of the University of Minnesota.
 * Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
 * associated documentation files (the "Software"), to deal in the Software without restriction,
 * including without limitation the rights to use, copy, modify, merge, publish, distribute,
 * sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions: The above copyright notice and this
 * permission notice shall be included in all copies of this Software or works derived from this
 * Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
 * INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE
 * AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
 * DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <proj.h>

#include "mapserver.h"

#ifndef FORAGER_MAPSERVER_COMMIT
#error "FORAGER_MAPSERVER_COMMIT names the MapServer build's commit; build-mapserver.sh sets it"
#endif

static void fail(const char *what, long line) {
  fprintf(stderr, "mapserver_proj: line %ld: %s\n", line, what);
  msWriteError(stderr);
  msCleanup();
  exit(1);
}

/* Loads `definition` into `p`, as msLoadProjectionString replaces what `p` held. */
static void load(projectionObj *p, const char *definition, long line) {
  if (msLoadProjectionString(p, definition) != MS_SUCCESS)
    fail("a projection does not load", line);
}

int main(int argc, char *argv[]) {
  if (argc != 2) {
    fprintf(stderr, "usage: mapserver_proj <points.tsv>\n");
    return 1;
  }
  FILE *list = fopen(argv[1], "r");
  if (!list) {
    perror(argv[1]);
    return 1;
  }
  if (msSetup() != MS_SUCCESS) {
    msWriteError(stderr);
    return 1;
  }
  msProjDataInitFromEnv();

  printf("# PROJ %s\n", proj_info().release);
  printf("# MapServer %s\n", FORAGER_MAPSERVER_COMMIT);

  projectionObj in, out;
  msInitProjection(&in);
  msInitProjection(&out);
  char *source = NULL, *target = NULL;
  char buffer[4096];
  long number = 0;
  while (fgets(buffer, sizeof buffer, list)) {
    number++;
    size_t length = strcspn(buffer, "\r\n");
    if (buffer[length] == '\0' && !feof(list))
      fail("the line is longer than the reader reads", number);
    buffer[length] = '\0';
    if (length == 0 || buffer[0] == '#')
      continue;
    char *line = strdup(buffer);
    char *fields[4];
    int count = 0;
    for (char *field = strtok(buffer, "\t"); field && count < 4; field = strtok(NULL, "\t"))
      fields[count++] = field;
    if (count != 4 || strtok(NULL, "\t"))
      fail("a line does not hold four tab-separated fields", number);
    if (!source || strcmp(source, fields[0]) != 0) {
      free(source);
      source = strdup(fields[0]);
      load(&in, source, number);
    }
    if (!target || strcmp(target, fields[1]) != 0) {
      free(target);
      target = strdup(fields[1]);
      load(&out, target, number);
    }
    pointObj point = {0};
    char *end;
    point.x = strtod(fields[2], &end);
    if (*end)
      fail("x is no number", number);
    point.y = strtod(fields[3], &end);
    if (*end)
      fail("y is no number", number);
    if (msProjectPoint(&in, &out, &point) == MS_SUCCESS)
      printf("%s\t%.17g\t%.17g\n", line, point.x, point.y);
    else
      printf("%s\tfail\n", line);
    free(line);
  }
  fclose(list);
  free(source);
  free(target);
  msFreeProjection(&in);
  msFreeProjection(&out);
  msCleanup();
  return 0;
}
