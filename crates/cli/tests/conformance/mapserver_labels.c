/*
 * The reader of MapServer's label cache for the noise measurement (ADR-0029): it renders a case
 * as map2img renders it, following main of src/apps/map2img.c at the pin for the options the
 * harness applies, -m, -o, -e, -s, and -l, and then writes each label of the cache to standard
 * output, one line per member, tab-separated, in the order msDrawLabelCache in src/mapdraw.c
 * walks them: from the highest priority down, and within a slot from the last added to the first.
 *
 * A line holds the priority, the layer's index, the layer's name, the class index, 1 when the
 * member is listed in the cache's rendered_text_symbols and 0 otherwise, the member's bbox as
 * "minx miny maxx maxy" for a drawn member and empty for a discarded one, and then each of its
 * texts in a field of its own. A tab, a line feed, a carriage return, and a backslash in a name
 * or a text are written as \t, \n, \r, and \\.
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

#include "mapserver.h"

/* Writes `s` with the escapes the header names. */
static void put_escaped(const char *s) {
  for (; s && *s; s++) {
    switch (*s) {
    case '\t':
      fputs("\\t", stdout);
      break;
    case '\n':
      fputs("\\n", stdout);
      break;
    case '\r':
      fputs("\\r", stdout);
      break;
    case '\\':
      fputs("\\\\", stdout);
      break;
    default:
      putchar(*s);
    }
  }
}

/* Whether `member` is listed among the cache's rendered members. */
static int drawn(const labelCacheObj *cache, const labelCacheMemberObj *member) {
  for (int i = 0; i < cache->num_rendered_members; i++) {
    if (cache->rendered_text_symbols[i] == member)
      return 1;
  }
  return 0;
}

static void fail(mapObj *map, configObj *config) {
  msWriteError(stderr);
  if (map)
    msFreeMap(map);
  msCleanup();
  msFreeConfig(config);
  exit(1);
}

int main(int argc, char *argv[]) {
  mapObj *map = NULL;
  const char *outfile = NULL;

  if (msSetup() != MS_SUCCESS) {
    msWriteError(stderr);
    exit(1);
  }
  configObj *config = msLoadConfig(NULL);
  msProjDataInitFromEnv();
  if (msDebugInitFromEnv() != MS_SUCCESS)
    fail(NULL, config);

  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "-m") == 0 && i + 1 < argc) {
      map = msLoadMap(argv[i + 1], NULL, config);
      if (!map)
        fail(NULL, config);
      msApplyDefaultSubstitutions(map);
      msApplyStyleItemsToLayers(map);
    }
  }
  if (!map) {
    fprintf(stderr, "Mapfile (-m) option not specified.\n");
    msCleanup();
    msFreeConfig(config);
    exit(1);
  }

  for (int i = 1; i < argc; i++) {
    if (strcmp(argv[i], "-o") == 0 && i + 1 < argc) {
      outfile = argv[++i];
    } else if (strcmp(argv[i], "-e") == 0 && i + 4 < argc) {
      map->extent.minx = atof(argv[i + 1]);
      map->extent.miny = atof(argv[i + 2]);
      map->extent.maxx = atof(argv[i + 3]);
      map->extent.maxy = atof(argv[i + 4]);
      i += 4;
    } else if (strcmp(argv[i], "-s") == 0 && i + 2 < argc) {
      msMapSetSize(map, atoi(argv[i + 1]), atoi(argv[i + 2]));
      i += 2;
    } else if (strcmp(argv[i], "-l") == 0 && i + 1 < argc) {
      int num_layers = 0;
      char **layers = msStringSplit(argv[i + 1], ' ', &num_layers);
      for (int j = 0; j < num_layers; j++) {
        int found = 0;
        for (int k = 0; k < map->numlayers && !found; k++) {
          layerObj *layer = GET_LAYER(map, k);
          found = (layer->name && strcasecmp(layer->name, layers[j]) == 0) ||
                  (layer->group && strcasecmp(layer->group, layers[j]) == 0);
        }
        if (!found) {
          fprintf(stderr, "Layer (-l) \"%s\" not found\n", layers[j]);
          msFreeCharArray(layers, num_layers);
          msFreeMap(map);
          msCleanup();
          msFreeConfig(config);
          exit(1);
        }
      }
      for (int j = 0; j < map->numlayers; j++) {
        layerObj *layer = GET_LAYER(map, j);
        if (layer->status == MS_DEFAULT)
          continue;
        layer->status = MS_OFF;
        for (int k = 0; k < num_layers; k++) {
          if ((layer->name && strcasecmp(layer->name, layers[k]) == 0) ||
              (layer->group && strcasecmp(layer->group, layers[k]) == 0)) {
            layer->status = MS_ON;
            break;
          }
        }
      }
      msFreeCharArray(layers, num_layers);
      i += 1;
    }
  }

  imageObj *image = msDrawMap(map, MS_FALSE);
  if (!image)
    fail(map, config);
  if (msSaveImage(map, image, outfile) != MS_SUCCESS)
    msWriteError(stderr);

  /* The cache stays filled until the next map is prepared (msPrepareImage). */
  const labelCacheObj *cache = &map->labelcache;
  for (int priority = MS_MAX_LABEL_PRIORITY - 1; priority >= 0; priority--) {
    const labelCacheSlotObj *slot = &cache->slots[priority];
    for (int l = slot->numlabels - 1; l >= 0; l--) {
      const labelCacheMemberObj *member = &slot->labels[l];
      const layerObj *layer = GET_LAYER(map, member->layerindex);
      int is_drawn = drawn(cache, member);
      printf("%d\t%d\t", priority + 1, member->layerindex);
      put_escaped(layer->name);
      printf("\t%d\t%d\t", member->classindex, is_drawn);
      if (is_drawn)
        printf("%.17g %.17g %.17g %.17g", member->bbox.minx, member->bbox.miny,
               member->bbox.maxx, member->bbox.maxy);
      for (int t = 0; t < member->numtextsymbols; t++) {
        putchar('\t');
        put_escaped(member->textsymbols[t]->annotext);
      }
      putchar('\n');
    }
  }

  msFreeImage(image);
  msFreeMap(map);
  msCleanup();
  msFreeConfig(config);
  return 0;
}
