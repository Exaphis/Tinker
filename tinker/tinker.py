import calendar
import datetime

import google.oauth2.credentials
import google_auth_oauthlib.flow
import googleapiclient.discovery

from tinker import app, flask

SCOPES = ['https://www.googleapis.com/auth/calendar', 'https://www.googleapis.com/auth/tasks.readonly']


@app.before_request
def before_request():
    if flask.request.url.startswith('http://'):
        url = flask.request.url.replace('http://', 'https://', 1)
        code = 301
        return flask.redirect(url, code=code)


@app.route('/')
def index():
    if 'credentials' not in flask.session:
        return flask.redirect(flask.url_for('authorize'))

    data = {}

    # Get date/time data
    now = datetime.datetime.now()
    data['year'] = now.year
    data['month'] = now.strftime('%B')
    data['day_int'] = now.day
    data['day_str'] = now.strftime('%A')
    data['first_day_of_month'], data['days_in_month'] = calendar.monthrange(now.year, now.month)
    data['first_day_of_month'] = (data['first_day_of_month'] + 1) % 7

    # Set up Google API credentials
    credentials = google.oauth2.credentials.Credentials(**flask.session['credentials'])

    # Get Google Calendar events
    cal_service = googleapiclient.discovery.build('calendar', 'v3', credentials=credentials)
    now = datetime.datetime.utcnow().isoformat() + 'Z'
    primary_events = cal_service.events().list(calendarId='primary', timeMin=now,
                                               maxResults=10, singleEvents=True,
                                               orderBy='startTime').execute()
    # holiday_events = service.events().list(calendarId='en.usa#holiday@group.v.calendar.google.com', timeMin=now,
    #                                        maxResults=10, singleEvents=True,
    #                                        orderBy='startTime').execute()

    events = primary_events.get('items', [])  # + holiday_events.get('items', [])
    # events.sort(key=lambda x: x['start']['date'] if 'date' in x['start'] else x['start']['dateTime'])

    # Within 6 days — day of week
    # Else - month day
    data['events'] = []
    for event in events:
        start_time = None
        if 'date' in event['start']:
            start_time = datetime.datetime.strptime(event['start']['date'], '%Y-%m-%d')
        elif 'dateTime' in event['start']:
            start_time = datetime.datetime.strptime(event['start']['dateTime'], '%Y-%m-%dT%H:%M:%S%z')

        time_str = ''
        if start_time:
            if 'date' in event['end']:
                end_time = datetime.datetime.strptime(event['end']['date'], '%Y-%m-%d')
                end_time -= datetime.timedelta(days=1)

                if end_time.date() == start_time.date():
                    time_str = end_time.strftime('%b %d')
                else:
                    time_str = f"{start_time.strftime('%b %d')} - {end_time.strftime('%b %d')}"
            elif 'dateTime' in event['end']:
                end_time = datetime.datetime.strptime(event['end']['dateTime'], '%Y-%m-%dT%H:%M:%S%z')
                if end_time.date() == start_time.date():
                    time_str = f"{start_time.strftime('%b %d')} " \
                               f"{start_time.strftime('%H:%S')} - {end_time.strftime('%H:%S')}"
                else:
                    time_str = f"{start_time.strftime('%b %d %H:%S')} - {start_time.strftime('%b %d %H:%s')}"

        if time_str:
            data['events'].append({'summary': event['summary'],
                                   'time': time_str})
        else:
            data['events'].append({'summary': event['summary']})

    for event in data['events']:
        print(event)
    print()

    # Get Google Tasks
    task_service = googleapiclient.discovery.build('tasks', 'v1', credentials=credentials)

    task_list = task_service.tasklists().list(maxResults=1).execute().get('items', [])
    if task_list:
        task_results = task_service.tasks().list(tasklist=task_list[0]['id'],
                                                 showCompleted=False).execute()
        tasks = task_results.get('items', [])

        data['tasks'] = []
        for task in tasks:
            if 'due' in task:
                date = datetime.datetime.strptime(task['due'], '%Y-%m-%dT%H:%M:%S.000Z')
                due_date = date.strftime('%b %d')

                data['tasks'].append({'title': task['title'],
                                      'due_date': due_date})
            else:
                data['tasks'].append({'title': task['title']})
    else:
        data['tasks'] = []

    for task in data['tasks']:
        print(task)

    return flask.render_template('index.html', **data)


@app.route("/authorize")
def authorize():
    # Taken from https://github.com/youtube/api-samples/blob/master/python/quickstart_web.py
    flow = google_auth_oauthlib.flow.Flow.from_client_secrets_file(
        'client_secrets.json',
        scopes=SCOPES,
        redirect_uri=flask.url_for("oauth2callback", _external=True))
    authorization_url, state = flow.authorization_url(
        access_type='offline',
        include_granted_scopes='true')

    flask.session['state'] = state
    flask.session['code_verifier'] = flow.code_verifier

    return flask.redirect(authorization_url)


@app.route('/oauth2callback')
def oauth2callback():
    # Taken from https://github.com/youtube/api-samples/blob/master/python/quickstart_web.py
    state = flask.session['state']
    flow = google_auth_oauthlib.flow.Flow.from_client_secrets_file(
        'client_secrets.json',
        scopes=SCOPES,
        state=state,
        redirect_uri=flask.url_for('oauth2callback', _external=True))
    flow.code_verifier = flask.session['code_verifier']

    authorization_response = flask.request.url
    flow.fetch_token(authorization_response=authorization_response)

    credentials = flow.credentials
    flask.session['credentials'] = {
        'token': credentials.token,
        'refresh_token': credentials.refresh_token,
        'token_uri': credentials.token_uri,
        'client_id': credentials.client_id,
        'client_secret': credentials.client_secret,
        'scopes': credentials.scopes
    }

    return flask.redirect(flask.url_for('index'))


if __name__ == '__main__':
    app.run(ssl_context='adhoc')
