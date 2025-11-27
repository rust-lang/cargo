// Position of the vertical axis.
const X_LINE = 50;
// General-use margin size.
const MARGIN = 5;
// Position of the horizontal axis, relative to the bottom.
const Y_LINE = 35;
// Minimum distance between time tick labels.
const MIN_TICK_DIST = 50;
// Radius for rounded rectangle corners.
const RADIUS = 3;
// Height of unit boxes.
const BOX_HEIGHT = 25;
// Distance between Y tick marks on the unit graph.
const Y_TICK_DIST = BOX_HEIGHT + 2;
// Rects used for mouseover detection.
// Objects of {x, y, x2, y2, i} where `i` is the index into UNIT_DATA.
let HIT_BOXES = [];
// Index into UNIT_DATA of the last unit hovered over by mouse.
let LAST_HOVER = null;
// Key is unit index, value is {x, y, width, sections} of the box.
let UNIT_COORDS = {};
// Map of unit index to the index it was unlocked by.
let REVERSE_UNIT_DEPS = {};
let REVERSE_UNIT_RMETA_DEPS = {};

const MIN_GRAPH_WIDTH = 200;
const MAX_GRAPH_WIDTH = 4096;

// How many pixels per second is added by each scale value
const SCALE_PIXELS_PER_SEC = 8;

function scale_to_graph_width(scale) {
  // The scale corresponds to `SCALE_PIXELS_PER_SEC` pixels per seconds.
  // We thus multiply it by that, and the total duration, to get the graph width.
  const width = scale * SCALE_PIXELS_PER_SEC * DURATION;

  // We then cap the size of the graph. It is hard to view if it is too large, and
  // browsers may not render a large graph because it takes too much memory.
  // 4096 is still ridiculously large, and probably won't render on mobile
  // browsers, but should be ok for many desktop environments.
  // Also use a minimum width of 200.
  return Math.max(MIN_GRAPH_WIDTH, Math.min(MAX_GRAPH_WIDTH, width));
}

// This function performs the reverse of `scale_to_graph_width`.
function width_to_graph_scale(width) {
  const maxWidth = Math.min(MAX_GRAPH_WIDTH, width);
  const minWidth = Math.max(MIN_GRAPH_WIDTH, width);

  const trimmedWidth = Math.max(minWidth, Math.min(maxWidth, width));

  const scale = Math.round(trimmedWidth / (DURATION * SCALE_PIXELS_PER_SEC));
  return Math.max(1, scale);
}

// Init scale value and limits based on the client's window width and min/max graph width.
const scaleElement = document.getElementById('scale');
scaleElement.min = width_to_graph_scale(MIN_GRAPH_WIDTH);
scaleElement.max = width_to_graph_scale(MAX_GRAPH_WIDTH);
scaleElement.value = width_to_graph_scale(window.innerWidth * 0.75);

// Colors from css
const getCssColor = name => getComputedStyle(document.body).getPropertyValue(name);
const TEXT_COLOR = getCssColor('--text');
const BG_COLOR = getCssColor('--background');
const CANVAS_BG = getCssColor('--canvas-background');
const AXES_COLOR = getCssColor('--canvas-axes');
const GRID_COLOR = getCssColor('--canvas-grid');
const CODEGEN_COLOR = getCssColor('--canvas-codegen');
const LINK_COLOR = getCssColor('--canvas-link');
// Final leftover section after link
const OTHER_COLOR = getCssColor('--canvas-other');
const CUSTOM_BUILD_COLOR = getCssColor('--canvas-custom-build');
const NOT_CUSTOM_BUILD_COLOR = getCssColor('--canvas-not-custom-build');
const DEP_LINE_COLOR = getCssColor('--canvas-dep-line');
const DEP_LINE_HIGHLIGHTED_COLOR = getCssColor('--canvas-dep-line-highlighted');
const CPU_COLOR = getCssColor('--canvas-cpu');

for (let n=0; n<UNIT_DATA.length; n++) {
  let unit = UNIT_DATA[n];
  for (let unlocked of unit.unlocked_units) {
    REVERSE_UNIT_DEPS[unlocked] = n;
  }
  for (let unlocked of unit.unlocked_rmeta_units) {
    REVERSE_UNIT_RMETA_DEPS[unlocked] = n;
  }
}

function render_pipeline_graph() {
  if (UNIT_DATA.length == 0) {
    return;
  }
  let g = document.getElementById('pipeline-graph');
  HIT_BOXES.length = 0;
  g.onmousemove = pipeline_mousemove;
  const min_time = document.getElementById('min-unit-time').valueAsNumber;

  const units = UNIT_DATA.filter(unit => unit.duration >= min_time);

  const graph_height = Y_TICK_DIST * units.length;
  const {ctx, graph_width, canvas_width, canvas_height, px_per_sec} = draw_graph_axes('pipeline-graph', graph_height);
  const container = document.getElementById('pipeline-container');
  container.style.width = canvas_width;
  container.style.height = canvas_height;

  // Canvas for hover highlights. This is a separate layer to improve performance.
  const linectx = setup_canvas('pipeline-graph-lines', canvas_width, canvas_height);
  linectx.clearRect(0, 0, canvas_width, canvas_height);
  ctx.strokeStyle = AXES_COLOR;
  // Draw Y tick marks.
  for (let n=1; n<units.length; n++) {
    const y = MARGIN + Y_TICK_DIST * n;
    ctx.beginPath();
    ctx.moveTo(X_LINE, y);
    ctx.lineTo(X_LINE-5, y);
    ctx.stroke();
  }

  // Draw Y labels.
  ctx.textAlign = 'end';
  ctx.textBaseline = 'middle';
  ctx.fillStyle = AXES_COLOR;
  for (let n=0; n<units.length; n++) {
    let y = MARGIN + Y_TICK_DIST * n + Y_TICK_DIST / 2;
    ctx.fillText(n+1, X_LINE-4, y);
  }

  // Draw the graph.
  ctx.save();
  ctx.translate(X_LINE, MARGIN);

  // Compute x,y coordinate of each block.
  // We also populate a map with the count of each unit name to disambiguate if necessary
  const unitCount = new Map();
  UNIT_COORDS = {};
  for (i=0; i<units.length; i++) {
    let unit = units[i];
    let y = i * Y_TICK_DIST + 1;
    let x = px_per_sec * unit.start;

    const sections = [];
    if (unit.sections !== null) {
        // We have access to compilation sections
        for (const section of unit.sections) {
            const [name, {start, end}] = section;
            sections.push({
                name,
                start: x + px_per_sec * start,
                width: (end - start) * px_per_sec
            });
        }
    }
    else if (unit.rmeta_time != null) {
        // We only know the rmeta time
        sections.push({
            name: "codegen",
            start: x + px_per_sec * unit.rmeta_time,
            width: (unit.duration - unit.rmeta_time) * px_per_sec
        });
    }
    let width = Math.max(px_per_sec * unit.duration, 1.0);
    UNIT_COORDS[unit.i] = {x, y, width, sections};

    const count = unitCount.get(unit.name) || 0;
    unitCount.set(unit.name, count + 1);
  }

  const presentSections = new Set();

  // Draw the blocks.
  for (i=0; i<units.length; i++) {
    let unit = units[i];
    let {x, y, width, sections} = UNIT_COORDS[unit.i];

    HIT_BOXES.push({x: X_LINE+x, y:MARGIN+y, x2: X_LINE+x+width, y2: MARGIN+y+BOX_HEIGHT, i: unit.i});

    ctx.beginPath();
    ctx.fillStyle = unit.mode == 'run-custom-build' ? CUSTOM_BUILD_COLOR : NOT_CUSTOM_BUILD_COLOR;
    roundedRect(ctx, x, y, width, BOX_HEIGHT, RADIUS);
    ctx.fill();

    for (const section of sections) {
        ctx.beginPath();
        ctx.fillStyle = get_section_color(section.name);
        roundedRect(ctx, section.start, y, section.width, BOX_HEIGHT, RADIUS);
        ctx.fill();
        presentSections.add(section.name);
    }
    ctx.fillStyle = TEXT_COLOR;
    ctx.textAlign = 'start';
    ctx.textBaseline = 'middle';
    ctx.font = '14px sans-serif';

    const labelName = (unitCount.get(unit.name) || 0) > 1 ? `${unit.name} (v${unit.version})${unit.target}` : `${unit.name}${unit.target}`;
    const label = `${labelName}: ${unit.duration}s`;

    const text_info = ctx.measureText(label);
    const label_x = Math.min(x + 5.0, canvas_width - text_info.width - X_LINE);
    ctx.fillText(label, label_x, y + BOX_HEIGHT / 2);
    draw_dep_lines(ctx, unit.i, false);
  }
  ctx.restore();

  // Draw a legend.
  ctx.save();
  ctx.translate(canvas_width - 200, MARGIN);

  const legend_entries = [{
    name: "Frontend/rest",
    color: NOT_CUSTOM_BUILD_COLOR,
    line: false
  }];
  if (presentSections.has("codegen")) {
    legend_entries.push({
      name: "Codegen",
      color: CODEGEN_COLOR,
      line: false
    });
  }
  if (presentSections.has("link")) {
    legend_entries.push({
      name: "Linking",
      color: LINK_COLOR,
      line: false
    });
  }
  if (presentSections.has("other")) {
    legend_entries.push({
      name: "Other",
      color: OTHER_COLOR,
      line: false
    });
  }
  draw_legend(ctx, 160, legend_entries);
  ctx.restore();
}

// Draw a legend at the current position of the ctx.
// entries should be an array of objects with the following scheme:
// {
//   "name": <name of the legend entry> [string],
//   "color": <color of the legend entry> [string],
//   "line": <should the entry be a thin line or a rectangle> [bool]
// }
function draw_legend(ctx, width, entries) {
  const entry_height = 20;

  // Add a bit of margin to the bottom and top
  const height = entries.length * entry_height + 4;

  // Draw background
  ctx.fillStyle = BG_COLOR;
  ctx.strokeStyle = TEXT_COLOR;
  ctx.lineWidth = 1;
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'start';
  ctx.beginPath();
  ctx.rect(0, 0, width, height);
  ctx.stroke();
  ctx.fill();

  ctx.lineWidth = 2;

  // Dimension of a block
  const block_height = 15;
  const block_width = 30;

  // Margin from the left edge
  const x_start = 5;
  // Width of the "mark" section (line/block)
  const mark_width = 45;

  // Draw legend entries
  let y = 12;
  for (const entry of entries) {
    ctx.beginPath();

    if (entry.line) {
      ctx.strokeStyle = entry.color;
      ctx.moveTo(x_start, y);
      ctx.lineTo(x_start + mark_width, y);
      ctx.stroke();
    } else {
      ctx.fillStyle = entry.color;
      ctx.fillRect(x_start + (mark_width - block_width) / 2, y - (block_height / 2), block_width, block_height);
    }

    ctx.fillStyle = TEXT_COLOR;
    ctx.fillText(entry.name, x_start + mark_width + 4, y + 1);

    y += entry_height;
  }
}

// Determine the color of a section block based on the section name.
function get_section_color(name) {
    if (name === "codegen") {
        return CODEGEN_COLOR;
    } else if (name === "link") {
        return LINK_COLOR;
    } else if (name === "other") {
        return OTHER_COLOR;
    } else {
        // We do not know what section this is, so just use the default color
        return NOT_CUSTOM_BUILD_COLOR;
    }
}

// Gets the x-coordinate of the codegen section of a unit.
//
// This is for drawing rmeta dependency lines.
function get_codegen_section_x(sections) {
    const codegen_section = sections.find(s => s.name === "codegen")
    if (!codegen_section) {
        // This happens only when type-checking (e.g., `cargo check`)
        return null;
    }
    return codegen_section.start;
}

// Draws lines from the given unit to the units it unlocks.
function draw_dep_lines(ctx, unit_idx, highlighted) {
  const unit = UNIT_DATA[unit_idx];
  const {x, y, sections} = UNIT_COORDS[unit_idx];
  ctx.save();
  for (const unlocked of unit.unlocked_units) {
    draw_one_dep_line(ctx, x, y, unlocked, highlighted);
  }
  for (const unlocked of unit.unlocked_rmeta_units) {
    const codegen_x = get_codegen_section_x(sections);
    draw_one_dep_line(ctx, codegen_x, y, unlocked, highlighted);
  }
  ctx.restore();
}

function draw_one_dep_line(ctx, from_x, from_y, to_unit, highlighted) {
  if (to_unit in UNIT_COORDS) {
    let {x: u_x, y: u_y} = UNIT_COORDS[to_unit];
    ctx.strokeStyle = highlighted ? DEP_LINE_HIGHLIGHTED_COLOR: DEP_LINE_COLOR;
    ctx.setLineDash([2]);
    ctx.beginPath();
    ctx.moveTo(from_x, from_y+BOX_HEIGHT/2);
    ctx.lineTo(from_x-5, from_y+BOX_HEIGHT/2);
    ctx.lineTo(from_x-5, u_y+BOX_HEIGHT/2);
    ctx.lineTo(u_x, u_y+BOX_HEIGHT/2);
    ctx.stroke();
  }
}

function render_timing_graph() {
  if (CONCURRENCY_DATA.length == 0) {
    return;
  }
  const HEIGHT = 400;
  const AXIS_HEIGHT = HEIGHT - MARGIN - Y_LINE;
  const TOP_MARGIN = 10;
  const GRAPH_HEIGHT = AXIS_HEIGHT - TOP_MARGIN;

  const {canvas_width, graph_width, ctx} = draw_graph_axes('timing-graph', AXIS_HEIGHT);

  // Draw Y tick marks and labels.
  let max_v = 0;
  for (c of CONCURRENCY_DATA) {
    max_v = Math.max(max_v, c.active, c.waiting, c.inactive);
  }
  const px_per_v = GRAPH_HEIGHT / max_v;
  const {step, tick_dist, num_ticks} = split_ticks(max_v, px_per_v, GRAPH_HEIGHT);
  ctx.textAlign = 'end';
  for (n=0; n<num_ticks; n++) {
    let y = HEIGHT - Y_LINE - ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(X_LINE, y);
    ctx.lineTo(X_LINE-5, y);
    ctx.stroke();
    ctx.fillText((n+1) * step, X_LINE-10, y+5);
  }

  // Label the Y axis.
  let label_y = (HEIGHT - Y_LINE) / 2;
  ctx.save();
  ctx.translate(15, label_y);
  ctx.rotate(3*Math.PI/2);
  ctx.textAlign = 'center';
  ctx.fillText('# Units', 0, 0);
  ctx.restore();

  // Draw the graph.
  ctx.save();
  ctx.translate(X_LINE, MARGIN);

  function coord(t, v) {
    return {
      x: graph_width * (t/DURATION),
      y: TOP_MARGIN + GRAPH_HEIGHT * (1.0 - (v / max_v))
    };
  }

  const cpuFillStyle = CPU_COLOR;
  if (CPU_USAGE.length > 1) {
    ctx.beginPath();
    ctx.fillStyle = cpuFillStyle;
    let bottomLeft = coord(CPU_USAGE[0][0], 0);
    ctx.moveTo(bottomLeft.x, bottomLeft.y);
    for (let i=0; i < CPU_USAGE.length; i++) {
      let [time, usage] = CPU_USAGE[i];
      let {x, y} = coord(time, usage / 100.0 * max_v);
      ctx.lineTo(x, y);
    }
    let bottomRight = coord(CPU_USAGE[CPU_USAGE.length - 1][0], 0);
    ctx.lineTo(bottomRight.x, bottomRight.y);
    ctx.fill();
  }

  function draw_line(style, key) {
    let first = CONCURRENCY_DATA[0];
    let last = coord(first.t, key(first));
    ctx.strokeStyle = style;
    ctx.beginPath();
    ctx.moveTo(last.x, last.y);
    for (let i=1; i<CONCURRENCY_DATA.length; i++) {
      let c = CONCURRENCY_DATA[i];
      let {x, y} = coord(c.t, key(c));
      ctx.lineTo(x, last.y);
      ctx.lineTo(x, y);
      last = {x, y};
    }
    ctx.stroke();
  }

  draw_line('blue', function(c) {return c.inactive;});
  draw_line('red', function(c) {return c.waiting;});
  draw_line('green', function(c) {return c.active;});

  // Draw a legend.
  ctx.restore();
  ctx.save();
  ctx.translate(canvas_width-200, MARGIN);
  draw_legend(ctx, 150, [{
    name: "Waiting",
    color: "red",
    line: true
  }, {
    name: "Inactive",
    color: "blue",
    line: true
  }, {
    name: "Active",
    color: "green",
    line: true
  }, {
    name: "CPU Usage",
    color: cpuFillStyle,
    line: false
  }]);
  ctx.restore();
}

function setup_canvas(id, width, height) {
  let g = document.getElementById(id);
  let dpr = window.devicePixelRatio || 1;
  g.width = width * dpr;
  g.height = height * dpr;
  g.style.width = width;
  g.style.height = height;
  let ctx = g.getContext('2d');
  ctx.scale(dpr, dpr);
  return ctx;
}

function draw_graph_axes(id, graph_height) {
  const scale = document.getElementById('scale').valueAsNumber;
  const graph_width = scale_to_graph_width(scale);
  const px_per_sec = graph_width / DURATION;
  const canvas_width = Math.max(graph_width + X_LINE + 30, X_LINE + 250);
  const canvas_height = graph_height + MARGIN + Y_LINE;
  let ctx = setup_canvas(id, canvas_width, canvas_height);
  ctx.fillStyle = CANVAS_BG;
  ctx.fillRect(0, 0, canvas_width, canvas_height);

  ctx.lineWidth = 2;
  ctx.font = '16px sans-serif';
  ctx.textAlign = 'center';
  ctx.strokeStyle = AXES_COLOR;

  // Draw main axes.
  ctx.beginPath();
  ctx.moveTo(X_LINE, MARGIN);
  ctx.lineTo(X_LINE, graph_height + MARGIN);
  ctx.lineTo(X_LINE+graph_width+20, graph_height + MARGIN);
  ctx.stroke();

  // Draw X tick marks.
  const {step, tick_dist, num_ticks} = split_ticks(DURATION, px_per_sec, graph_width);
  ctx.fillStyle = AXES_COLOR;
  for (let n=0; n<num_ticks; n++) {
    const x = X_LINE + ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(x, canvas_height-Y_LINE);
    ctx.lineTo(x, canvas_height-Y_LINE+5);
    ctx.stroke();

    ctx.fillText(`${(n+1) * step}s`, x, canvas_height - Y_LINE + 20);
  }

  // Draw vertical lines.
  ctx.strokeStyle = GRID_COLOR;
  ctx.setLineDash([2, 4]);
  for (n=0; n<num_ticks; n++) {
    const x = X_LINE + ((n + 1) * tick_dist);
    ctx.beginPath();
    ctx.moveTo(x, MARGIN);
    ctx.lineTo(x, MARGIN+graph_height);
    ctx.stroke();
  }
  ctx.strokeStyle = TEXT_COLOR;
  ctx.setLineDash([]);
  return {canvas_width, canvas_height, graph_width, graph_height, ctx, px_per_sec};
}

// Determine the spacing and number of ticks along an axis.
function split_ticks(max_value, px_per_v, max_px) {
  const max_ticks = Math.floor(max_px / MIN_TICK_DIST);
  if (max_ticks <= 1) {
    // Graph is too small for even 1 tick.
    return {step: max_value, tick_dist: max_px, num_ticks: 1};
  }
  let step;
  if (max_value <= max_ticks) {
    step = 1;
  } else if (max_value <= max_ticks * 2) {
    step = 2;
  } else if (max_value <= max_ticks * 4) {
    step = 4;
  } else if (max_value <= max_ticks * 5) {
    step = 5;
  } else {
    step = 10;
    let count = 0;
    while (true) {
      if (count > 100) {
        throw Error("tick loop too long");
      }
      count += 1;
      if (max_value <= max_ticks * step) {
        break;
      }
      step += 10;
    }
  }
  const tick_dist = px_per_v * step;
  const num_ticks = Math.floor(max_value / step);
  return {step, tick_dist, num_ticks};
}

function codegen_time(unit) {
  if (unit.rmeta_time == null) {
    return null;
  }
  let ctime = unit.duration - unit.rmeta_time;
  return [unit.rmeta_time, ctime];
}

function roundedRect(ctx, x, y, width, height, r) {
  r = Math.min(r, width, height);
  ctx.beginPath();
  ctx.moveTo(x+r, y);
  ctx.lineTo(x+width-r, y);
  ctx.arc(x+width-r, y+r, r, 3*Math.PI/2, 0);
  ctx.lineTo(x+width, y+height-r);
  ctx.arc(x+width-r, y+height-r, r, 0, Math.PI/2);
  ctx.lineTo(x+r, y+height);
  ctx.arc(x+r, y+height-r, r, Math.PI/2, Math.PI);
  ctx.lineTo(x, y-r);
  ctx.arc(x+r, y+r, r, Math.PI, 3*Math.PI/2);
  ctx.closePath();
}

function pipeline_mouse_hit(event) {
  // This brute-force method can be optimized if needed.
  for (let box of HIT_BOXES) {
    if (event.offsetX >= box.x && event.offsetX <= box.x2 &&
        event.offsetY >= box.y && event.offsetY <= box.y2) {
      return box;
    }
  }
}

function pipeline_mousemove(event) {
  // Highlight dependency lines on mouse hover.
  let box = pipeline_mouse_hit(event);
  if (box) {
    if (box.i != LAST_HOVER) {
      LAST_HOVER = box.i;
      let g = document.getElementById('pipeline-graph-lines');
      let ctx = g.getContext('2d');
      ctx.clearRect(0, 0, g.width, g.height);
      ctx.save();
      ctx.translate(X_LINE, MARGIN);
      ctx.lineWidth = 2;
      draw_dep_lines(ctx, box.i, true);

      if (box.i in REVERSE_UNIT_DEPS) {
        const dep_unit = REVERSE_UNIT_DEPS[box.i];
        if (dep_unit in UNIT_COORDS) {
          const {x, y} = UNIT_COORDS[dep_unit];
          draw_one_dep_line(ctx, x, y, box.i, true);
        }
      }
      if (box.i in REVERSE_UNIT_RMETA_DEPS) {
        const dep_unit = REVERSE_UNIT_RMETA_DEPS[box.i];
        if (dep_unit in UNIT_COORDS) {
          const {y, sections} = UNIT_COORDS[dep_unit];
          const codegen_x = get_codegen_section_x(sections);
          draw_one_dep_line(ctx, codegen_x, y, box.i, true);
        }
      }
      ctx.restore();
    }
  }
}

render_pipeline_graph();
render_timing_graph();

// Set up and handle controls.
{
  const range = document.getElementById('min-unit-time');
  const time_output = document.getElementById('min-unit-time-output');
  time_output.innerHTML = `${range.value}s`;
  range.oninput = event => {
    time_output.innerHTML = `${range.value}s`;
    render_pipeline_graph();
  };

  const scale = document.getElementById('scale');
  const scale_output = document.getElementById('scale-output');
  scale_output.innerHTML = `${scale.value}`;
  scale.oninput = event => {
    scale_output.innerHTML = `${scale.value}`;
    render_pipeline_graph();
    render_timing_graph();
  };
}
